//! GitHub sync: commits the open notes folder and syncs its current branch with a Git remote.
//!
//! The notes folder must be the root of a Git repository. Quire stages every change, commits it,
//! rebases onto the remote branch and pushes. Git's own credentials, SSH agent and remote do the
//! authentication. Unlike the object-store sync, this keeps the repository's history, and it does
//! not sync pins or offer encryption: locked notes are still committed as sealed files.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::{Outcome, Remote};

/// A credential prompt or a stalled network connection must not hold a sync forever.
const TIMEOUT: Duration = Duration::from_secs(180);

struct GitOutput {
    ok: bool,
    out: String,
    err: String,
}

/// Runs Git in `root` with no terminal prompt. Output is small for every command used here.
fn git_raw(root: &Path, args: &[&str]) -> Result<GitOutput, String> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("LC_ALL", "C")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "Git is not installed. Install Git to sync this folder.".to_string()
            } else {
                format!("Could not run Git: {e}")
            }
        })?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() > TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Git did not finish within three minutes. Check your credentials and connection, then try again.".into());
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(format!("Could not wait for Git: {e}"));
            }
        }
    }
    let out = child.wait_with_output().map_err(|e| format!("Could not read Git's output: {e}"))?;
    Ok(GitOutput {
        ok: out.status.success(),
        out: String::from_utf8_lossy(&out.stdout).trim().to_string(),
        err: String::from_utf8_lossy(&out.stderr).trim().to_string(),
    })
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let r = git_raw(root, args)?;
    if r.ok { Ok(r.out) } else { Err(r.err) }
}

/// Removes a user name, password or token from a URL, so an error or status cannot show it.
fn scrub(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("://") {
        out.push_str(&rest[..i + 3]);
        rest = &rest[i + 3..];
        let end = rest.find('/').unwrap_or(rest.len());
        if let Some(at) = rest[..end].rfind('@') {
            rest = &rest[at + 1..];
        }
    }
    out.push_str(rest);
    out
}

fn detail(e: &str) -> String {
    let text = e.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
    scrub(&text)
}

fn remote_name(r: &Remote) -> Result<String, String> {
    let name = r.val("remote", "origin");
    if name.is_empty() || name.starts_with('-') || name.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("The Git remote name is not valid.".into());
    }
    Ok(name)
}

struct Repo {
    root: PathBuf,
    remote: String,
    branch: String,
    url: String,
}

impl Repo {
    fn open(r: &Remote, root: &Path) -> Result<Repo, String> {
        let root = std::fs::canonicalize(root).map_err(|e| format!("Could not open the notes folder: {e}"))?;
        let inside = git(&root, &["rev-parse", "--is-inside-work-tree"])
            .map_err(|_| "The notes folder is not a Git repository. Run git init in it, or open a folder that is one.".to_string())?;
        if inside != "true" {
            return Err("The notes folder is not a Git working tree. Open the folder you cloned from GitHub.".into());
        }
        let top = git(&root, &["rev-parse", "--show-toplevel"]).map_err(|e| format!("Git could not find the repository root: {}", detail(&e)))?;
        let top = std::fs::canonicalize(top).map_err(|e| format!("Could not open the repository root: {e}"))?;
        if top != root {
            return Err(format!(
                "The notes folder must be the root of its Git repository. The repository root is {}.",
                top.display()
            ));
        }
        let branch = git(&root, &["symbolic-ref", "--short", "HEAD"]).map_err(|_| {
            "The Git repository is not on a branch. Check out the branch you want to sync, then test again.".to_string()
        })?;
        let remote = remote_name(r)?;
        let url = git(&root, &["remote", "get-url", &remote])
            .map_err(|_| format!("The Git remote {remote} is not set in this repository."))?;
        Ok(Repo { root, remote, branch, url })
    }

    fn display(&self) -> String {
        let url = scrub(&self.url);
        if url.is_empty() { format!("{} ({})", self.remote, self.branch) } else { format!("{url} ({})", self.branch) }
    }

    fn pull(&self) -> Result<(), String> {
        let branch_ref = format!("refs/heads/{}", self.branch);
        let remote_has_branch = git(&self.root, &["ls-remote", "--heads", &self.remote, &branch_ref]).map(|s| !s.is_empty());
        let found = match remote_has_branch {
            Ok(v) => v,
            Err(e) => return Err(format!("Could not reach {}: {}", self.display(), detail(&e))),
        };
        if !found {
            return Ok(());
        }
        match git(&self.root, &["pull", "--rebase", "--quiet", &self.remote, &self.branch]) {
            Ok(_) => Ok(()),
            Err(e) => {
                let _ = git(&self.root, &["rebase", "--abort"]);
                let d = detail(&e);
                if d.contains("CONFLICT") || d.contains("could not apply") || d.contains("conflict") {
                    Err(format!(
                        "Git found a conflict with {}. Your notes are committed here. Resolve the conflict in the repository, then sync again.",
                        self.display()
                    ))
                } else {
                    Err(format!("Could not update from {}: {d}", self.display()))
                }
            }
        }
    }

    fn push(&self) -> Result<(), String> {
        let refspec = format!("HEAD:{}", self.branch);
        let first = match git(&self.root, &["push", "--quiet", &self.remote, &refspec]) {
            Ok(_) => return Ok(()),
            Err(e) => e,
        };
        // Another device may have pushed between the rebase and this push. Fetch and try once more.
        let d = detail(&first);
        let again = d.contains("non-fast-forward") || d.contains("fetch first") || d.contains("rejected");
        if again && self.pull().is_ok() && git(&self.root, &["push", "--quiet", &self.remote, &refspec]).is_ok() {
            return Ok(());
        }
        Err(format!("Could not push to {}: {d}", self.display()))
    }
}

/// Keeps Quire's local trash out of the repository on this computer only.
fn ignore_trash(root: &Path) -> Result<(), String> {
    let rel = git(root, &["rev-parse", "--git-path", "info/exclude"])?;
    let path = PathBuf::from(rel.trim());
    let path = if path.is_absolute() { path } else { root.join(path) };
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    if current.lines().any(|l| l.trim() == ".trash/") {
        return Ok(());
    }
    let mut next = current;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(".trash/\n");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Could not write Git's exclude file: {e}"))?;
    }
    std::fs::write(&path, next).map_err(|e| format!("Could not write Git's exclude file: {e}"))
}

/// Checks the folder and the remote, as "Test connection" does.
pub fn test(r: &Remote, root: &Path) -> Result<String, String> {
    let repo = Repo::open(r, root)?;
    if let Err(e) = git(&repo.root, &["var", "GIT_AUTHOR_IDENT"]) {
        return Err(format!(
            "Git has no user name and email for commits. Set user.name and user.email, then test again. {}",
            detail(&e)
        ));
    }
    let r = git_raw(&repo.root, &["ls-remote", "--heads", &repo.remote, &format!("refs/heads/{}", repo.branch)])?;
    if !r.ok {
        return Err(format!("Could not reach {}: {}", repo.display(), detail(&r.err)));
    }
    Ok(if r.out.is_empty() {
        format!("Connected to {}. The first sync pushes branch {}.", scrub(&repo.url), repo.branch)
    } else {
        format!("Connected to {} on branch {}.", scrub(&repo.url), repo.branch)
    })
}

/// Commits the notes folder, rebases it onto the remote branch and pushes it.
pub fn run(r: &Remote, root: &Path, device: &str) -> Result<Outcome, String> {
    let repo = Repo::open(r, root)?;
    ignore_trash(&repo.root)?;

    git(&repo.root, &["add", "-A"]).map_err(|e| format!("Could not stage the notes: {}", detail(&e)))?;
    let staged = git(&repo.root, &["diff", "--cached", "--name-only"]).map_err(|e| format!("Could not read what changed: {}", detail(&e)))?;
    let committed = staged.lines().filter(|l| !l.trim().is_empty()).count();
    if committed > 0 {
        let message = format!("Quire: notes from {device}");
        git(&repo.root, &["commit", "--quiet", "-m", &message]).map_err(|e| {
            format!(
                "Could not commit the notes. Check Git's user.name and user.email. {}",
                detail(&e)
            )
        })?;
    }

    repo.pull()?;
    repo.push()?;

    Ok(Outcome { place: repo.display(), root: root.to_path_buf(), uploaded: committed, ..Default::default() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_at(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn remote() -> Remote {
        Remote {
            provider: "github".into(),
            cfg: std::collections::BTreeMap::from([("remote".to_string(), "origin".to_string())]),
            prefix: String::new(),
            encrypt: false,
            pass: String::new(),
        }
    }

    fn repo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("quire-git-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn init(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        git_at(dir, &["init", "-q", "-b", "main"]);
        git_at(dir, &["config", "user.name", "Quire Test"]);
        git_at(dir, &["config", "user.email", "quire@example.invalid"]);
    }

    #[test]
    fn a_folder_that_is_not_a_repository_is_refused() {
        let dir = repo("plain");
        std::fs::create_dir_all(&dir).unwrap();
        let err = test(&remote(), &dir).unwrap_err();
        assert!(err.contains("not a Git repository"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn commits_and_pushes_and_pulls_another_device() {
        let base = repo("sync");
        let bare = base.join("remote.git");
        let one = base.join("one");
        let two = base.join("two");
        std::fs::create_dir_all(&bare).unwrap();
        git_at(&base, &["init", "-q", "--bare", "-b", "main", bare.to_str().unwrap()]);
        init(&one);
        std::fs::write(one.join("A.md"), "one\n").unwrap();
        std::fs::create_dir_all(one.join(".trash")).unwrap();
        std::fs::write(one.join(".trash/Deleted.md"), "gone\n").unwrap();
        git_at(&one, &["remote", "add", "origin", bare.to_str().unwrap()]);

        let out = run(&remote(), &one, "laptop").unwrap();
        assert_eq!(out.uploaded, 1);
        assert!(test(&remote(), &one).unwrap().contains("main"));
        assert_eq!(git_at(&bare, &["show", "main:A.md"]), "one");
        let files = git_at(&bare, &["ls-tree", "-r", "--name-only", "main"]);
        assert!(!files.contains(".trash"), "Quire's trash must stay out of Git: {files}");

        // A second device clones, changes B, and pushes.
        let two_s = two.to_str().unwrap();
        git_at(&base, &["clone", "-q", bare.to_str().unwrap(), two_s]);
        git_at(&two, &["config", "user.name", "Quire Test"]);
        git_at(&two, &["config", "user.email", "quire@example.invalid"]);
        std::fs::write(two.join("B.md"), "two\n").unwrap();
        git_at(&two, &["add", "-A"]);
        git_at(&two, &["commit", "-q", "-m", "B"]);
        git_at(&two, &["push", "-q", "origin", "main"]);

        run(&remote(), &one, "laptop").unwrap();
        assert_eq!(std::fs::read_to_string(one.join("B.md")).unwrap(), "two\n");

        // No changes makes no commit.
        let before = git_at(&one, &["rev-list", "--count", "HEAD"]);
        run(&remote(), &one, "laptop").unwrap();
        assert_eq!(git_at(&one, &["rev-list", "--count", "HEAD"]), before);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_conflict_leaves_the_local_commit_and_stops() {
        let base = repo("conflict");
        let bare = base.join("remote.git");
        let one = base.join("one");
        let two = base.join("two");
        std::fs::create_dir_all(&bare).unwrap();
        git_at(&base, &["init", "-q", "--bare", "-b", "main", bare.to_str().unwrap()]);
        init(&one);
        std::fs::write(one.join("A.md"), "base\n").unwrap();
        git_at(&one, &["remote", "add", "origin", bare.to_str().unwrap()]);
        run(&remote(), &one, "one").unwrap();

        git_at(&base, &["clone", "-q", bare.to_str().unwrap(), two.to_str().unwrap()]);
        git_at(&two, &["config", "user.name", "Quire Test"]);
        git_at(&two, &["config", "user.email", "quire@example.invalid"]);
        std::fs::write(two.join("A.md"), "remote\n").unwrap();
        git_at(&two, &["add", "-A"]);
        git_at(&two, &["commit", "-q", "-m", "remote"]);
        git_at(&two, &["push", "-q", "origin", "main"]);

        std::fs::write(one.join("A.md"), "local\n").unwrap();
        let err = run(&remote(), &one, "one").unwrap_err();
        assert!(err.contains("conflict"), "{err}");
        assert_eq!(std::fs::read_to_string(one.join("A.md")).unwrap(), "local\n");
        assert_eq!(git(&one, &["status", "--porcelain"]).unwrap(), "");

        let _ = std::fs::remove_dir_all(&base);
    }
}
