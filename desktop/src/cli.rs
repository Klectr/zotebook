//! Command-line arguments: `zotebook [folder]` and `zotebook mcp --folder <path>`.

use std::path::PathBuf;

/// A folder, or a note in a folder, named on the command line. It opens instead of the last
/// folder. For a note, the second value is its path inside the folder.
pub fn start_target() -> Option<(PathBuf, Option<String>)> {
    if rune::is_headless() {
        return std::env::var_os("ZOTEBOOK_FOLDER").map(|p| (PathBuf::from(p), None));
    }
    let arg = rune::extra_args().iter().map(PathBuf::from).find(|p| p.exists())?.canonicalize().ok()?;
    if arg.is_dir() {
        return Some((arg, None));
    }
    let is_md = arg.extension().is_some_and(|e| e.eq_ignore_ascii_case("md"));
    let dir = arg.parent()?.to_path_buf();
    let name = arg.file_name()?.to_string_lossy().into_owned();
    Some((dir, is_md.then_some(name)))
}

/// Runs `zotebook mcp`, if that is what was asked. Returns false to start the app instead.
pub fn run_subcommand() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("mcp") => {
            let folder = args.iter().position(|a| a == "--folder").and_then(|i| args.get(i + 1)).map(PathBuf::from);
            let Some(folder) = folder else {
                eprintln!("Usage: zotebook mcp --folder <path>");
                std::process::exit(2);
            };
            if !folder.is_dir() {
                eprintln!("zotebook mcp: {} is not a folder", folder.display());
                std::process::exit(2);
            }
            if let Err(e) = crate::mcp::serve_stdio(folder) {
                eprintln!("zotebook mcp: {e}");
                std::process::exit(1);
            }
            true
        }
        Some("--version" | "-V") => {
            println!("zotebook {}", crate::update::CURRENT);
            true
        }
        Some("--help" | "-h") => {
            println!("Zotebook, a plain editor for folders of Markdown files.\n\nUsage:\n  zotebook [folder]               Open the app, optionally on a folder\n  zotebook mcp --folder <path>    Serve a folder to AI apps over MCP (stdio)\n  zotebook --version              Print the version");
            true
        }
        _ => false,
    }
}
