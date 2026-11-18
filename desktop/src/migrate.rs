//! Zotebook was called Quire. The first run after the rename moves Quire's settings and data
//! folders to Zotebook's, so the notes folders, tabs, themes and sync keys carry over.
//!
//! - Saved paths that pointed inside a moved folder, such as the sample folder in the data
//!   folder, point at the new place.
//! - A notes folder that synced to the old default remote folder, `quire/<folder name>/`, keeps
//!   that folder, and its last-sync record moves with it, so the next sync does not start over.
//! - Locked notes and the files in a remote folder keep their format, so Quire and Zotebook
//!   read each other's notes.

use std::path::{Path, PathBuf};

use rune::dirs::AppDirs;

use crate::store::{self, Persist};

/// The app's id and the remote folder root before the rename.
const OLD_ID: &str = "quire";

/// Moves Quire's folders to Zotebook's on the first run after the rename. Does nothing once
/// Zotebook has its own state, or when `ZOTEBOOK_STATE` points somewhere else.
pub fn from_quire() {
    if std::env::var_os("ZOTEBOOK_STATE").is_some() {
        return;
    }
    let Some(new) = store::dirs() else { return };
    let old = new.sibling(OLD_ID);
    if let Err(e) = run(&old, &new) {
        eprintln!("zotebook: could not move Quire's settings: {e}");
    }
}

fn run(old: &AppDirs, new: &AppDirs) -> std::io::Result<bool> {
    let (old_state, new_state) = (old.config().join("state.json"), new.config().join("state.json"));
    if new_state.exists() || !old_state.exists() {
        return Ok(false);
    }
    let mut p: Persist = serde_json::from_slice(&std::fs::read(&old_state)?).unwrap_or_default();

    // Keep the remote folder of every folder that used the old default, while it still has
    // its old path (the settings are keyed by path).
    if p.settings.sync.provider != "none" {
        for v in &p.vaults {
            let key = v.path.to_string_lossy().into_owned();
            let set = p.settings.sync.folders.get(&key).is_some_and(|f| !f.trim().is_empty());
            if !set {
                p.settings.sync.folders.insert(key, format!("{OLD_ID}/{}/", v.name));
            }
        }
    }
    let old_ids: Vec<(PathBuf, Option<String>)> = p.vaults.iter().map(|v| (v.path.clone(), record_id(&p, &v.path, &v.name))).collect();

    // Move each folder, unless Zotebook already has one there. On macOS several are the same.
    let pairs = [(old.config(), new.config()), (old.data(), new.data()), (old.state(), new.state()), (old.cache(), new.cache()), (old.logs(), new.logs())];
    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (from, to) in pairs {
        if from == to || !from.exists() || to.exists() {
            continue;
        }
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(from, to)?;
        moved.push((from.to_path_buf(), to.to_path_buf()));
    }
    let remap = |path: &Path| -> PathBuf {
        moved.iter().find_map(|(from, to)| path.strip_prefix(from).ok().map(|rest| to.join(rest))).unwrap_or_else(|| path.to_path_buf())
    };
    let remap_key = |key: String| remap(Path::new(&key)).to_string_lossy().into_owned();

    for v in &mut p.vaults {
        v.path = remap(&v.path);
    }
    p.last = p.last.as_deref().map(remap);
    p.sessions = std::mem::take(&mut p.sessions).into_iter().map(|(k, s)| (remap_key(k), s)).collect();
    p.settings.sync.folders = std::mem::take(&mut p.settings.sync.folders).into_iter().map(|(k, f)| (remap_key(k), f)).collect();

    // A last-sync record is named after its folder's path, so a moved folder's record moves.
    let records = new.config().join("sync");
    for ((old_path, old_id), v) in old_ids.iter().zip(&p.vaults) {
        let (Some(old_id), Some(new_id)) = (old_id, record_id(&p, &v.path, &v.name)) else { continue };
        if *old_path != v.path && *old_id != new_id {
            let _ = std::fs::rename(records.join(format!("{old_id}.json")), records.join(format!("{new_id}.json")));
        }
    }

    // The config folder held the old state file; it is replaced by the one with new paths.
    let stale = remap(&old_state);
    store::save_at(&new_state, &p)?;
    if stale != new_state {
        let _ = std::fs::remove_file(stale);
    }
    Ok(true)
}

fn record_id(p: &Persist, path: &Path, name: &str) -> Option<String> {
    crate::sync::remote_for(&p.settings.sync, path, name).map(|r| r.id(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::VaultRef;

    #[test]
    fn quire_settings_move_to_zotebook() {
        let root = std::env::temp_dir().join(format!("zotebook-migrate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (old, new) = (AppDirs::rooted(&root, OLD_ID), AppDirs::rooted(&root, "zotebook"));

        // Quire's state: a sample folder inside its data folder, and a folder elsewhere with
        // its own remote folder, both syncing.
        let sample = old.data().join("Field Notes");
        let elsewhere = root.join("Documents/Work");
        std::fs::create_dir_all(&sample).unwrap();
        std::fs::write(sample.join("Welcome.md"), "# Welcome").unwrap();
        let mut p = Persist::default();
        p.vaults = vec![VaultRef { name: "Field Notes".into(), path: sample.clone() }, VaultRef { name: "Work".into(), path: elsewhere.clone() }];
        p.last = Some(sample.clone());
        p.sessions.insert(sample.to_string_lossy().into_owned(), Default::default());
        p.settings.sync.provider = "s3".into();
        p.settings.sync.folders.insert(elsewhere.to_string_lossy().into_owned(), "mine/work/".into());
        p.settings.theme = "dark".into();
        store::save_at(&old.config().join("state.json"), &p).unwrap();
        // The record as Quire named it: the old default remote folder, the old path.
        let mut quire = p.clone();
        quire.settings.sync.folders.insert(sample.to_string_lossy().into_owned(), "quire/Field Notes/".into());
        let sample_record = record_id(&quire, &sample, "Field Notes").unwrap();
        std::fs::create_dir_all(old.config().join("sync")).unwrap();
        std::fs::write(old.config().join("sync").join(format!("{sample_record}.json")), "{}").unwrap();

        assert!(run(&old, &new).unwrap());
        let got: Persist = serde_json::from_slice(&std::fs::read(new.config().join("state.json")).unwrap()).unwrap();
        let moved = new.data().join("Field Notes");
        assert_eq!(got.vaults[0].path, moved);
        assert!(moved.join("Welcome.md").exists(), "the sample folder moved with the data folder");
        assert_eq!(got.vaults[1].path, elsewhere, "a folder outside Quire's folders stays put");
        assert_eq!(got.last.as_ref(), Some(&moved));
        assert!(got.sessions.contains_key(&*moved.to_string_lossy()));
        assert_eq!(got.settings.theme, "dark");
        let folders = &got.settings.sync.folders;
        assert_eq!(folders.get(&*moved.to_string_lossy()).map(String::as_str), Some("quire/Field Notes/"), "the old default remote folder is kept");
        assert_eq!(folders.get(&*elsewhere.to_string_lossy()).map(String::as_str), Some("mine/work/"));
        let new_record = record_id(&got, &moved, "Field Notes").unwrap();
        assert!(new.config().join("sync").join(format!("{new_record}.json")).exists(), "the last-sync record follows the folder");
        assert!(!old.config().exists() && !old.data().exists());

        // A second run leaves Zotebook's state alone.
        assert!(!run(&old, &new).unwrap());
        let _ = std::fs::remove_dir_all(&root);
    }
}
