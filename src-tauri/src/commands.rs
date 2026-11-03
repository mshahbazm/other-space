//! Everything the window can ask the backend to do (Tauri commands), plus the
//! helpers the menu bar shares with them.

use crate::accounts::{self, AccountStore, Folder, DEFAULT_ID};
use crate::{claude, claude_app, license, settings, shell, terminal, tray};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Manager, State};

pub struct AppState {
    pub store: Mutex<AccountStore>,
    pub license: Mutex<license::Licensing>,
}

#[derive(Serialize)]
pub struct AccountView {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    /// Where this account's Claude Code data lives.
    pub data_dir: String,
    pub email: Option<String>,
    pub organization: Option<String>,
}

fn view(store: &AccountStore, id: &str) -> Option<AccountView> {
    let name = store.name(id)?;
    let signed_in = claude::signed_in(&store.state_file(id));
    Some(AccountView {
        id: id.to_string(),
        name,
        is_default: id == DEFAULT_ID,
        data_dir: store.data_dir_of(id).to_string_lossy().into_owned(),
        email: signed_in.email,
        organization: signed_in.organization,
    })
}

/// The default account first, then every added account.
pub fn account_views(store: &AccountStore) -> Vec<AccountView> {
    std::iter::once(DEFAULT_ID.to_string())
        .chain(store.list().into_iter().map(|a| a.id))
        .filter_map(|id| view(store, &id))
        .collect()
}

/// Write (or refresh) an account's activate script and return its path.
fn activate_script(store: &AccountStore, id: &str) -> Result<PathBuf, String> {
    let name = store.name(id).ok_or_else(|| format!("No account '{id}'"))?;
    terminal::write_activate_script(&name, store.config_dir(id).as_deref(), &store.script_dir(id))
}

/// Open Terminal as an account, optionally in a folder, and start Claude
/// Code. Quitting `claude` leaves a shell that's still on that account.
pub fn open_claude_code_as(store: &AccountStore, id: &str, folder: Option<&Path>) -> Result<(), String> {
    let script = activate_script(store, id)?;
    terminal::open_terminal(&script, folder)
}

/// Open the Claude desktop app as an account.
pub fn open_claude_app_as(store: &AccountStore, id: &str) -> Result<(), String> {
    if !store.exists(id) {
        return Err(format!("No account '{id}'"));
    }
    claude_app::open(store.config_dir(id).as_deref(), store.claude_app_dir(id).as_deref())
}

/// Regenerate everything the zsh hook consumes: activate scripts for every
/// account that owns a folder, the folder→account map, and the hook itself.
pub fn sync_shell(store: &AccountStore) -> Result<(), String> {
    let mut entries = Vec::new();
    for folder in store.folders() {
        if store.exists(&folder.account_id) {
            entries.push((folder.path.clone(), activate_script(store, &folder.account_id)?));
        }
    }
    shell::write_folder_map(&entries)?;
    shell::write_hook()
}

// ---------- Accounts ----------

#[tauri::command]
pub fn list_accounts(state: State<AppState>) -> Vec<AccountView> {
    account_views(&state.store.lock().unwrap())
}

#[tauri::command]
pub fn create_account(app: tauri::AppHandle, state: State<AppState>, name: String) -> Result<(), String> {
    state.store.lock().unwrap().create(&name)?;
    tray::refresh(&app);
    Ok(())
}

/// Remove an account from Other. With `delete_data`, its folder (login
/// state, chats, settings) also goes to the Trash and its saved login is
/// removed from the Keychain; otherwise everything stays on disk and adding
/// an account with the same name picks it back up.
#[tauri::command]
pub fn delete_account(
    app: tauri::AppHandle,
    state: State<AppState>,
    id: String,
    delete_data: bool,
) -> Result<String, String> {
    let msg = {
        let mut store = state.store.lock().unwrap();
        let name = store.name(&id).unwrap_or_else(|| id.clone());
        let config_dir = store.config_dir(&id);
        if delete_data && claude_app::is_running(store.claude_app_dir(&id).as_deref()) {
            return Err(format!("Quit “{name}”'s Claude app first, then remove it"));
        }
        let root = store.remove(&id)?;
        sync_shell(&store)?;
        if delete_data {
            if let Some(dir) = &config_dir {
                claude::forget_login(dir);
            }
            accounts::move_to_trash(&root)?;
            format!("Removed “{name}” and moved its data to the Trash")
        } else {
            format!("Removed “{name}” — its data is kept at {}", root.display())
        }
    };
    tray::refresh(&app);
    Ok(msg)
}

#[tauri::command]
pub fn open_claude_code(state: State<AppState>, id: String) -> Result<(), String> {
    open_claude_code_as(&state.store.lock().unwrap(), &id, None)
}

#[tauri::command]
pub fn open_claude_app(state: State<AppState>, id: String) -> Result<(), String> {
    open_claude_app_as(&state.store.lock().unwrap(), &id)
}

/// Copy settings (CLAUDE.md, settings.json, agents, commands, skills, hooks,
/// plugins, MCP servers) from one account into another. Logins and chats are
/// never copied.
#[tauri::command]
pub fn copy_settings(state: State<AppState>, from_id: String, to_id: String) -> Result<String, String> {
    if from_id == to_id {
        return Err("Pick a different account to copy from".into());
    }
    let store = state.store.lock().unwrap();
    for id in [&from_id, &to_id] {
        if !store.exists(id) {
            return Err(format!("No account '{id}'"));
        }
    }
    let res = settings::copy_settings(
        &store.data_dir_of(&from_id),
        &store.state_file(&from_id),
        &store.data_dir_of(&to_id),
        &store.state_file(&to_id),
    )?;
    let plural = |n: usize| if n == 1 { "" } else { "s" };
    let mut msg = format!("Copied {} file{}", res.copied, plural(res.copied));
    if res.kept > 0 {
        msg.push_str(&format!(", {} already up to date", res.kept));
    }
    if res.mcp_servers > 0 {
        msg.push_str(&format!(", {} MCP server{}", res.mcp_servers, plural(res.mcp_servers)));
    }
    Ok(msg)
}

// ---------- Folders ----------

#[tauri::command]
pub fn list_folders(state: State<AppState>) -> Vec<Folder> {
    state.store.lock().unwrap().folders()
}

#[tauri::command]
pub fn add_folder(
    app: tauri::AppHandle,
    state: State<AppState>,
    path: String,
    account_id: String,
) -> Result<Folder, String> {
    let folder = {
        let mut store = state.store.lock().unwrap();
        let folder = store.add_folder(&path, &account_id)?;
        sync_shell(&store)?;
        folder
    };
    tray::refresh(&app);
    Ok(folder)
}

#[tauri::command]
pub fn set_folder_account(
    app: tauri::AppHandle,
    state: State<AppState>,
    id: String,
    account_id: String,
) -> Result<(), String> {
    {
        let mut store = state.store.lock().unwrap();
        store.set_folder_account(&id, &account_id)?;
        sync_shell(&store)?;
    }
    tray::refresh(&app);
    Ok(())
}

#[tauri::command]
pub fn remove_folder(app: tauri::AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    {
        let mut store = state.store.lock().unwrap();
        store.remove_folder(&id)?;
        sync_shell(&store)?;
    }
    tray::refresh(&app);
    Ok(())
}

/// Start Claude Code in a folder, as the folder's account.
#[tauri::command]
pub fn open_folder(state: State<AppState>, id: String) -> Result<(), String> {
    let store = state.store.lock().unwrap();
    let folder = store
        .get_folder(&id)
        .ok_or_else(|| format!("No folder '{id}'"))?
        .clone();
    open_claude_code_as(&store, &folder.account_id, Some(Path::new(&folder.path)))
}

/// What the window needs to word things for this computer.
#[derive(Serialize)]
pub struct PlatformInfo {
    /// "macos", "windows" or "linux".
    pub os: &'static str,
    pub claude_installed: bool,
    pub claude_app_installed: bool,
    pub shell: shell::ShellInfo,
}

#[tauri::command]
pub fn platform_info() -> PlatformInfo {
    PlatformInfo {
        os: std::env::consts::OS,
        claude_installed: claude::resolve_cli().is_some(),
        claude_app_installed: claude_app::installed(),
        shell: shell::info(),
    }
}

#[tauri::command]
pub fn install_shell_integration(state: State<AppState>) -> Result<shell::ShellInfo, String> {
    let store = state.store.lock().unwrap();
    sync_shell(&store)?;
    shell::install()?;
    Ok(shell::info())
}

// ---------- License ----------

#[tauri::command]
pub fn license_status(state: State<AppState>) -> license::Status {
    state.license.lock().unwrap().status()
}

/// Activate a license key on this computer. The network call runs without
/// holding the lock.
#[tauri::command]
pub async fn activate_license(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    key: String,
) -> Result<license::Status, String> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("Enter your license key".into());
    }
    let (device_id, device_name) = state.license.lock().unwrap().device();
    let k = key.clone();
    let activated = tauri::async_runtime::spawn_blocking(move || license::activate(&k, &device_id, &device_name))
        .await
        .map_err(|e| e.to_string())??;
    let status = {
        let mut lic = state.license.lock().unwrap();
        lic.set_licensed(key, activated)?;
        lic.status()
    };
    tray::refresh(&app);
    Ok(status)
}

/// Free this computer's activation (to move the license elsewhere).
#[tauri::command]
pub async fn deactivate_license(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<license::Status, String> {
    let activation = state.license.lock().unwrap().activation();
    if let Some((key, id)) = activation {
        tauri::async_runtime::spawn_blocking(move || license::deactivate(&key, &id))
            .await
            .map_err(|e| e.to_string())??;
    }
    let status = {
        let mut lic = state.license.lock().unwrap();
        lic.clear_license()?;
        lic.status()
    };
    tray::refresh(&app);
    Ok(status)
}

#[tauri::command]
pub fn buy_license() -> Result<(), String> {
    license::open_url(license::BUY_URL)
}

/// Re-check a stored license in the background when it's due. Only a clear
/// "not valid" from the server removes it; being offline never does.
pub fn revalidate_license_in_background(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let activation = {
            let lic = state.license.lock().unwrap();
            if !lic.revalidation_due() {
                return;
            }
            lic.activation()
        };
        let Some((key, id)) = activation else { return };
        match license::validate(&key, &id) {
            Ok(Some(email)) => {
                let _ = state.license.lock().unwrap().mark_validated(email);
            }
            Ok(None) => {
                let _ = state.license.lock().unwrap().clear_license();
                let h = app.clone();
                let _ = app.run_on_main_thread(move || tray::refresh(&h));
            }
            Err(_) => {}
        }
    });
}

