//! Everything Other knows about Claude Code itself: where its CLI lives,
//! which account a config dir is signed in to, and where the login is kept.
//!
//! Isolation is a single env var: `CLAUDE_CONFIG_DIR`. Claude Code keeps its
//! state file at `$CLAUDE_CONFIG_DIR/.claude.json`. Its login is stored in the
//! macOS Keychain as "Claude Code-credentials-<hash of config dir>" (verified
//! on 2.1.209), and on Windows/Linux in `$CLAUDE_CONFIG_DIR/.credentials.json`
//! — so each account's `/login` is separate on every platform. The default
//! account is simply Claude Code with the variable unset (`~/.claude` +
//! `~/.claude.json`).

use crate::paths;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const CONFIG_ENV: &str = "CLAUDE_CONFIG_DIR";

/// Where to find `claude` when it's not on PATH. Relative to the home folder;
/// one `/*/` wildcard segment is allowed (newest match wins, for versioned
/// dirs).
#[cfg(target_os = "macos")]
const CLI_FALLBACKS: &[&str] = &[
    ".local/bin/claude",
    ".claude/local/claude",
    // The Claude desktop app manages versioned copies of the CLI.
    "Library/Application Support/Claude/claude-code/*/claude.app/Contents/MacOS/claude",
];
#[cfg(target_os = "linux")]
const CLI_FALLBACKS: &[&str] = &[".local/bin/claude", ".claude/local/claude"];
#[cfg(windows)]
const CLI_FALLBACKS: &[&str] = &[".local/bin/claude.exe", "AppData/Roaming/npm/claude.cmd"];

/// Where the `claude` CLI is available on this machine.
pub enum CliLocation {
    /// On the user's PATH — their shell finds it by name.
    OnPath,
    /// Only at a fallback location; terminals need it added to PATH.
    At(PathBuf),
}

#[cfg(unix)]
fn on_path() -> bool {
    // The user's login shell, so their real PATH (nvm, homebrew,
    // ~/.local/bin) applies — the app itself starts with a minimal PATH.
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    Command::new(shell)
        .args(["-lc", "command -v claude"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn on_path() -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    Command::new("where.exe")
        .arg("claude")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Resolve one fallback pattern against the home folder; among wildcard
/// matches the most recently modified wins.
fn resolve_fallback(pattern: &str) -> Option<PathBuf> {
    let home = paths::home();
    let Some((base, suffix)) = pattern.split_once("/*/") else {
        let p = home.join(pattern);
        return p.is_file().then_some(p);
    };
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(home.join(base)).ok()?.flatten() {
        let candidate = entry.path().join(suffix);
        if candidate.is_file() {
            let modified = candidate
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            if best.as_ref().map(|(t, _)| modified > *t).unwrap_or(true) {
                best = Some((modified, candidate));
            }
        }
    }
    best.map(|(_, p)| p)
}

pub fn resolve_cli() -> Option<CliLocation> {
    if on_path() {
        return Some(CliLocation::OnPath);
    }
    CLI_FALLBACKS
        .iter()
        .find_map(|p| resolve_fallback(p))
        .map(CliLocation::At)
}

/// The Keychain service holding a config dir's login: "Claude Code-credentials-"
/// plus the first 8 hex digits of SHA-256 of the config dir path.
#[cfg(target_os = "macos")]
fn keychain_service(config_dir: &Path) -> Option<String> {
    use std::io::Write;
    let mut child = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .ok()?;
    child
        .stdin
        .take()?
        .write_all(config_dir.to_string_lossy().as_bytes())
        .ok()?;
    let out = child.wait_with_output().ok()?;
    let hex = String::from_utf8_lossy(&out.stdout);
    let short = hex.get(..8)?;
    Some(format!("Claude Code-credentials-{short}"))
}

/// Remove a config dir's saved login from the Keychain (best effort; macOS
/// may ask the user to allow it).
#[cfg(target_os = "macos")]
pub fn forget_login(config_dir: &Path) {
    if let Some(service) = keychain_service(config_dir) {
        let _ = Command::new("/usr/bin/security")
            .args(["delete-generic-password", "-s", &service])
            .output();
    }
}

/// On Windows and Linux the login is a file inside the config dir, so
/// removing the account's folder already signs it out.
#[cfg(not(target_os = "macos"))]
pub fn forget_login(_config_dir: &Path) {}

/// Who a config dir is signed in as, read from Claude Code's own state file.
#[derive(Serialize, Clone, Default)]
pub struct SignedIn {
    pub email: Option<String>,
    pub organization: Option<String>,
}

pub fn signed_in(state_file: &Path) -> SignedIn {
    let Some(account) = std::fs::read_to_string(state_file)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|j| j.get("oauthAccount").cloned())
    else {
        return SignedIn::default();
    };
    let field = |k: &str| {
        account
            .get(k)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    SignedIn {
        email: field("emailAddress"),
        organization: field("organizationName"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_signed_in_account_or_nothing() {
        let dir = std::env::temp_dir().join("other-signedin");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join(".claude.json");
        std::fs::write(
            &f,
            r#"{"oauthAccount":{"emailAddress":"me@acme.com","organizationName":"Acme"}}"#,
        )
        .unwrap();
        let s = signed_in(&f);
        assert_eq!(s.email.as_deref(), Some("me@acme.com"));
        assert_eq!(s.organization.as_deref(), Some("Acme"));

        std::fs::write(&f, "{}").unwrap();
        assert!(signed_in(&f).email.is_none());
        assert!(signed_in(&dir.join("missing.json")).email.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Matches the scheme seen in a real Keychain (sha256 → first 8 hex).
    #[cfg(target_os = "macos")]
    #[test]
    fn keychain_service_name_format() {
        let service = keychain_service(Path::new("/Users/x/.multiapp/profiles/w/home/.claude")).unwrap();
        assert_eq!(service.len(), "Claude Code-credentials-".len() + 8);
    }
}
