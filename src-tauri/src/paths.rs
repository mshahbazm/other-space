//! Locations shared across the app.

use std::path::PathBuf;

/// The user's home folder. `HOME` first (also lets tests sandbox it), then
/// `USERPROFILE`, which is what Windows sets.
pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// Other's own folder: account data, activate scripts and shell hooks.
pub fn other_dir() -> PathBuf {
    home().join(".multiapp")
}

/// Expand a leading `~/` (or `~\`) to the home folder.
pub fn expand_tilde(path: &str) -> PathBuf {
    match path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        Some(rest) => home().join(rest),
        None => PathBuf::from(path),
    }
}

/// Drop Windows' `\\?\` verbatim prefix that `canonicalize` adds, so paths
/// compare equal to what shells report.
pub fn display_path(path: &std::path::Path) -> String {
    let s = path.to_string_lossy();
    s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
}
