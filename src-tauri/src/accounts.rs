//! Account storage. Besides the always-present default account (Claude Code's
//! own `~/.claude`, never touched), every account is a directory under
//! `~/.multiapp/profiles/<id>/` whose `home/.claude` is used as
//! `CLAUDE_CONFIG_DIR`, plus a list of folders mapped to accounts.

use crate::paths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Reserved id of the default account (no `CLAUDE_CONFIG_DIR`).
pub const DEFAULT_ID: &str = "default";
pub const DEFAULT_NAME: &str = "Default";

#[derive(Serialize, Deserialize, Clone)]
pub struct Account {
    pub id: String,
    pub name: String,
}

/// A folder owned by an account: entering it (shell integration) or opening
/// it from Other uses that account.
#[derive(Serialize, Deserialize, Clone)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub path: String,
    /// Serialized as `profile_id` for compatibility with existing data.
    #[serde(rename = "profile_id")]
    pub account_id: String,
}

/// On-disk format (`profiles.json`). Field names predate the "account" and
/// "folder" naming and are kept so existing installs keep their data.
#[derive(Serialize, Deserialize, Default)]
struct RegistryFile {
    #[serde(rename = "profiles")]
    accounts: Vec<Account>,
    #[serde(default, rename = "projects")]
    folders: Vec<Folder>,
}

pub struct AccountStore {
    data_dir: PathBuf,
    accounts: Vec<Account>,
    folders: Vec<Folder>,
}


impl AccountStore {
    pub fn load(data_dir: PathBuf) -> Self {
        let file = fs::read_to_string(data_dir.join("profiles.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<RegistryFile>(&s).ok())
            .unwrap_or_default();
        Self {
            data_dir,
            accounts: file.accounts,
            folders: file.folders,
        }
    }

    /// The account's directory (the on-disk path keeps its original name).
    fn account_root(&self, id: &str) -> PathBuf {
        paths::other_dir().join("profiles").join(id)
    }

    fn save(&self) -> Result<(), String> {
        fs::create_dir_all(&self.data_dir).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(&RegistryFile {
            accounts: self.accounts.clone(),
            folders: self.folders.clone(),
        })
        .map_err(|e| e.to_string())?;
        fs::write(self.data_dir.join("profiles.json"), json).map_err(|e| e.to_string())
    }

    pub fn list(&self) -> Vec<Account> {
        self.accounts.clone()
    }

    pub fn get(&self, id: &str) -> Option<&Account> {
        self.accounts.iter().find(|p| p.id == id)
    }

    /// True for the default account or any added account.
    pub fn exists(&self, id: &str) -> bool {
        id == DEFAULT_ID || self.get(id).is_some()
    }

    pub fn name(&self, id: &str) -> Option<String> {
        if id == DEFAULT_ID {
            return Some(DEFAULT_NAME.into());
        }
        self.get(id).map(|p| p.name.clone())
    }

    /// The `CLAUDE_CONFIG_DIR` an account runs with; None for the default.
    pub fn config_dir(&self, id: &str) -> Option<PathBuf> {
        (id != DEFAULT_ID).then(|| self.account_root(id).join("home").join(".claude"))
    }

    /// The Claude desktop app's `--user-data-dir` (its own login); None for
    /// the default account, which uses the app's normal location.
    pub fn claude_app_dir(&self, id: &str) -> Option<PathBuf> {
        // macOS keeps the path earlier versions created, so existing
        // accounts stay signed in to the app.
        let rel = if cfg!(target_os = "macos") {
            "home/Library/Application Support/Claude"
        } else {
            "home/claude-app"
        };
        (id != DEFAULT_ID).then(|| self.account_root(id).join(rel))
    }

    /// The directory holding the account's Claude Code data, default included.
    pub fn data_dir_of(&self, id: &str) -> PathBuf {
        self.config_dir(id)
            .unwrap_or_else(|| paths::home().join(".claude"))
    }

    /// Claude Code's state file (login identity, MCP servers, project index).
    pub fn state_file(&self, id: &str) -> PathBuf {
        match self.config_dir(id) {
            Some(dir) => dir.join(".claude.json"),
            None => paths::home().join(".claude.json"),
        }
    }

    /// Where the account's activate script is written.
    pub fn script_dir(&self, id: &str) -> PathBuf {
        if id == DEFAULT_ID {
            paths::other_dir().join("default")
        } else {
            self.account_root(id)
        }
    }

    pub fn create(&mut self, name: &str) -> Result<Account, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Account name is empty".into());
        }
        let base: String = name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let base = if base.is_empty() { "account".into() } else { base };
        let mut id = base.clone();
        let mut n = 2;
        while id == DEFAULT_ID || self.accounts.iter().any(|p| p.id == id) {
            id = format!("{base}-{n}");
            n += 1;
        }

        let account = Account {
            id: id.clone(),
            name: name.to_string(),
        };
        if let Some(dir) = self.config_dir(&id) {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        self.accounts.push(account.clone());
        self.save()?;
        Ok(account)
    }

    /// Removes the account from the registry. Its data directory is kept on
    /// disk so logins/sessions are never destroyed by accident.
    pub fn remove(&mut self, id: &str) -> Result<PathBuf, String> {
        if id == DEFAULT_ID {
            return Err("The default account can't be removed".into());
        }
        let before = self.accounts.len();
        self.accounts.retain(|p| p.id != id);
        if self.accounts.len() == before {
            return Err(format!("No account with id '{id}'"));
        }
        self.folders.retain(|f| f.account_id != id);
        self.save()?;
        Ok(self.account_root(id))
    }

    pub fn folders(&self) -> Vec<Folder> {
        self.folders.clone()
    }

    pub fn get_folder(&self, id: &str) -> Option<&Folder> {
        self.folders.iter().find(|p| p.id == id)
    }

    pub fn add_folder(&mut self, path: &str, account_id: &str) -> Result<Folder, String> {
        let dir = paths::expand_tilde(path.trim());
        if !dir.is_dir() {
            return Err(format!("Not a folder: {}", dir.display()));
        }
        if !dir.is_absolute() {
            return Err("Folder path must be absolute".into());
        }
        if !self.exists(account_id) {
            return Err(format!("No account '{account_id}'"));
        }
        let canonical = dir.canonicalize().map_err(|e| e.to_string())?;
        let canonical_str = paths::display_path(&canonical);
        if let Some(existing) = self.folders.iter_mut().find(|p| p.path == canonical_str) {
            existing.account_id = account_id.to_string();
            let updated = existing.clone();
            self.save()?;
            return Ok(updated);
        }
        let name = canonical
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| canonical_str.clone());
        let mut n = self.folders.len() + 1;
        let mut id = format!("proj-{n}");
        while self.folders.iter().any(|p| p.id == id) {
            n += 1;
            id = format!("proj-{n}");
        }
        let folder = Folder {
            id,
            name,
            path: canonical_str,
            account_id: account_id.to_string(),
        };
        self.folders.push(folder.clone());
        self.save()?;
        Ok(folder)
    }

    pub fn set_folder_account(&mut self, id: &str, account_id: &str) -> Result<(), String> {
        if !self.exists(account_id) {
            return Err(format!("No account '{account_id}'"));
        }
        let folder = self
            .folders
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("No folder '{id}'"))?;
        folder.account_id = account_id.to_string();
        self.save()
    }

    pub fn remove_folder(&mut self, id: &str) -> Result<(), String> {
        let before = self.folders.len();
        self.folders.retain(|p| p.id != id);
        if self.folders.len() == before {
            return Err(format!("No folder '{id}'"));
        }
        self.save()
    }
}

/// Move a file or folder to the Trash / Recycle Bin (recoverable), not
/// delete it.
pub fn move_to_trash(path: &Path) -> Result<(), String> {
    trash::delete(path).map_err(|e| format!("Couldn't move {} to the bin: {e}", path.display()))
}


#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox() -> PathBuf {
        let dir = std::env::temp_dir().join("multiapp-storetest");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        std::env::set_var("HOME", &dir);
        dir
    }

    #[test]
    fn account_and_folder_lifecycle() {
        let _guard = crate::test_env_lock();
        let root = sandbox();
        let mut store = AccountStore::load(root.join("data"));

        // Slugging + collision handling; "default" is reserved.
        let a = store.create("Client A!").unwrap();
        assert_eq!(a.id, "client-a");
        let a2 = store.create("Client A").unwrap();
        assert_eq!(a2.id, "client-a-2");
        assert_eq!(store.create("Default").unwrap().id, "default-2");
        assert!(store.config_dir(&a.id).unwrap().is_dir());
        assert!(store.create("   ").is_err());

        // The default account is Claude Code's own ~/.claude, no env override.
        assert!(store.config_dir(DEFAULT_ID).is_none());
        assert_eq!(store.state_file(DEFAULT_ID), root.join(".claude.json"));
        assert!(store.remove(DEFAULT_ID).is_err());

        // Folder mapping: add, re-add reassigns, default is a valid owner.
        let proj_dir = root.join("repo");
        fs::create_dir_all(&proj_dir).unwrap();
        let p = store.add_folder(proj_dir.to_str().unwrap(), &a.id).unwrap();
        assert_eq!(p.account_id, a.id);
        let p_again = store.add_folder(proj_dir.to_str().unwrap(), DEFAULT_ID).unwrap();
        assert_eq!(p_again.id, p.id);
        assert_eq!(p_again.account_id, DEFAULT_ID);
        assert_eq!(store.folders().len(), 1);
        assert!(store.add_folder("/nope/definitely/missing", &a.id).is_err());
        assert!(store.add_folder(proj_dir.to_str().unwrap(), "ghost").is_err());

        // Removing an account drops its folders but keeps its data on disk.
        store.set_folder_account(&p.id, &a2.id).unwrap();
        let kept = store.remove(&a2.id).unwrap();
        assert!(store.folders().is_empty());
        assert!(kept.is_dir(), "account data must survive removal");
        assert!(store.remove("ghost").is_err());

        // Persistence round-trip
        let reloaded = AccountStore::load(root.join("data"));
        assert_eq!(reloaded.list().len(), 2);
        assert_eq!(reloaded.list()[0].id, "client-a");
    }

    #[test]
    fn loads_the_original_on_disk_format() {
        let _guard = crate::test_env_lock();
        let root = sandbox();
        let data = root.join("data");
        fs::create_dir_all(&data).unwrap();
        fs::write(
            data.join("profiles.json"),
            r#"{"profiles":[{"id":"work","name":"Work"}],"projects":[{"id":"proj-1","name":"repo","path":"/r","profile_id":"work"}]}"#,
        )
        .unwrap();
        let store = AccountStore::load(data.clone());
        assert_eq!(store.list()[0].name, "Work");
        assert_eq!(store.folders()[0].account_id, "work");
        // And it's written back in the same format.
        let mut store = store;
        store.create("Other One").unwrap();
        let raw = fs::read_to_string(data.join("profiles.json")).unwrap();
        assert!(raw.contains("\"profiles\"") && raw.contains("\"projects\"") && raw.contains("\"profile_id\""));
    }
}
