//! "Copy settings" between accounts: the configuration you built up, never
//! identity or history.

use std::fs;
use std::path::Path;

/// What "Copy settings" brings across: configuration you built up, never
/// identity or history. Everything else in a config dir (credentials, chats,
/// caches, telemetry) stays put, so each account keeps its own login.
pub const SETTINGS_ENTRIES: &[&str] = &[
    "settings.json",
    "CLAUDE.md",
    "keybindings.json",
    "agents",
    "commands",
    "skills",
    "hooks",
    "output-styles",
    "plugins",
];

pub struct CopyResult {
    pub copied: usize,
    /// Files already identical in the destination — left untouched.
    pub kept: usize,
    /// User-level MCP servers merged into the destination's state file.
    pub mcp_servers: usize,
}

/// Copy settings from one account's config dir into another's: the
/// [`SETTINGS_ENTRIES`] files, plus user-level MCP servers from the state
/// file. MCP servers are merged in without overwriting ones the destination
/// already defines, and the destination's login (`oauthAccount`) is never
/// touched.
pub fn copy_settings(
    src_dir: &Path,
    src_state: &Path,
    dest_dir: &Path,
    dest_state: &Path,
) -> Result<CopyResult, String> {
    fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    let mut result = CopyResult { copied: 0, kept: 0, mcp_servers: 0 };
    for name in SETTINGS_ENTRIES {
        let src = src_dir.join(name);
        if src.symlink_metadata().is_ok() {
            merge_entry(&src, &dest_dir.join(name), &mut result)?;
        }
    }

    let read_json = |p: &Path| {
        fs::read_to_string(p)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
    };
    let src_mcp = read_json(src_state)
        .and_then(|j| j.get("mcpServers").and_then(|m| m.as_object().cloned()))
        .unwrap_or_default();
    if !src_mcp.is_empty() {
        let mut dest = read_json(dest_state).unwrap_or_else(|| serde_json::json!({}));
        let obj = dest
            .as_object_mut()
            .ok_or("The destination state file isn't a JSON object")?;
        let servers = obj
            .entry("mcpServers")
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .ok_or("The destination's mcpServers isn't a JSON object")?;
        for (k, v) in src_mcp {
            if !servers.contains_key(&k) {
                servers.insert(k, v);
                result.mcp_servers += 1;
            }
        }
        if result.mcp_servers > 0 {
            let json = serde_json::to_string_pretty(&dest).map_err(|e| e.to_string())?;
            fs::write(dest_state, json).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(dest_state, fs::Permissions::from_mode(0o600));
            }
        }
    }
    Ok(result)
}

/// Recursively copy `src` into `dest`. A file is copied when it's missing in
/// the destination or its size differs; identical-size files are left alone.
/// Symlinks are recreated only when absent (on Windows, where creating
/// symlinks needs extra rights, the file or folder they point to is copied).
fn merge_entry(src: &Path, dest: &Path, result: &mut CopyResult) -> Result<(), String> {
    #[cfg(windows)]
    let meta = fs::metadata(src).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    let meta = fs::symlink_metadata(src).map_err(|e| e.to_string())?;
    let ft = meta.file_type();
    #[cfg(unix)]
    if ft.is_symlink() {
        if dest.symlink_metadata().is_err() {
            let target = fs::read_link(src).map_err(|e| e.to_string())?;
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::os::unix::fs::symlink(target, dest).map_err(|e| e.to_string())?;
            result.copied += 1;
        } else {
            result.kept += 1;
        }
        return Ok(());
    }
    if ft.is_dir() {
        fs::create_dir_all(dest).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
            merge_entry(&entry.path(), &dest.join(entry.file_name()), result)?;
        }
    } else {
        let differs = match fs::metadata(dest) {
            Ok(d) => d.len() != meta.len(),
            Err(_) => true,
        };
        if differs {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(src, dest).map_err(|e| format!("Copying {} failed: {e}", src.display()))?;
            result.copied += 1;
        } else {
            result.kept += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sandbox() -> PathBuf {
        let dir = std::env::temp_dir().join("other-settings-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn copy_settings_never_carries_login_or_history() {
        let root = sandbox();
        let src = root.join("src/.claude");
        let dest = root.join("dest/.claude");
        fs::create_dir_all(src.join("agents")).unwrap();
        fs::create_dir_all(src.join("projects/-p")).unwrap();
        fs::write(src.join("agents/reviewer.md"), "agent").unwrap();
        fs::write(src.join("settings.json"), "{\"model\":\"opus\"}").unwrap();
        fs::write(src.join("CLAUDE.md"), "rules").unwrap();
        fs::write(src.join(".credentials.json"), "SECRET").unwrap();
        fs::write(src.join("projects/-p/chat.jsonl"), "chat").unwrap();
        fs::write(src.join("history.jsonl"), "history").unwrap();
        let src_state = root.join("src/.claude.json");
        fs::write(
            &src_state,
            r#"{"oauthAccount":{"emailAddress":"me@home.com"},"mcpServers":{"a":{"command":"x"},"b":{"command":"y"}}}"#,
        )
        .unwrap();
        fs::create_dir_all(&dest).unwrap();
        let dest_state = dest.join(".claude.json");
        fs::write(
            &dest_state,
            r#"{"oauthAccount":{"emailAddress":"me@work.com"},"mcpServers":{"b":{"command":"mine"}}}"#,
        )
        .unwrap();

        let res = copy_settings(&src, &src_state, &dest, &dest_state).unwrap();
        assert_eq!(res.copied, 3);
        assert_eq!(res.mcp_servers, 1, "only the missing server is added");
        assert!(dest.join("agents/reviewer.md").is_file());
        assert!(dest.join("CLAUDE.md").is_file());
        assert!(!dest.join(".credentials.json").exists(), "login must never copy");
        assert!(!dest.join("projects").exists(), "chats stay with their account");
        assert!(!dest.join("history.jsonl").exists());

        let state: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&dest_state).unwrap()).unwrap();
        assert_eq!(state["oauthAccount"]["emailAddress"], "me@work.com");
        assert_eq!(state["mcpServers"]["b"]["command"], "mine");
        assert_eq!(state["mcpServers"]["a"]["command"], "x");
    }
}
