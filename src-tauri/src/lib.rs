//! Other — run multiple Claude Code accounts side by side on one computer.
//!
//! - `accounts`   the account list and where each account's data lives
//! - `claude`     Claude Code itself: CLI lookup, signed-in account, login
//! - `claude_app` the Claude desktop app, one instance per account
//! - `terminal`   Claude Code in a terminal as an account
//! - `shell`      the optional shell hook that switches accounts by folder
//! - `paths`      shared locations (home folder, Other's data folder)
//! - `settings`   copying settings between accounts
//! - `license`    trial and license keys for official builds
//! - `commands`   what the window can call; `tray` the menu-bar menu

mod accounts;
mod claude;
mod claude_app;
mod commands;
mod license;
mod paths;
mod settings;
mod shell;
mod terminal;
mod tray;

use accounts::AccountStore;
use commands::AppState;
use std::sync::Mutex;
use tauri::Manager;

/// Tests that sandbox the HOME env var must hold this lock — cargo runs
/// tests in parallel and HOME is process-global.
#[cfg(test)]
pub(crate) fn test_env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Earlier bundle identifiers, newest first. The app-data dir is named after
/// the identifier, so each rename would otherwise lose the account list.
const LEGACY_IDENTIFIERS: &[&str] = &["space.myother.app", "com.xvelopers.multiapp"];

/// One-time carry-over: copy the registry from the newest earlier identifier's
/// data dir, so existing accounts/folders survive a rename. (Account data
/// itself lives under ~/.multiapp and is unaffected.)
fn migrate_legacy_data_dir(new_dir: &std::path::Path) {
    if new_dir.join("profiles.json").exists() {
        return;
    }
    let Some(legacy) = new_dir.parent().and_then(|parent| {
        LEGACY_IDENTIFIERS
            .iter()
            .map(|id| parent.join(id).join("profiles.json"))
            .find(|p| p.exists())
    }) else {
        return;
    };
    let _ = std::fs::create_dir_all(new_dir);
    let _ = std::fs::copy(legacy, new_dir.join("profiles.json"));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            migrate_legacy_data_dir(&data_dir);
            let license = license::Licensing::load(&data_dir);
            let store = AccountStore::load(data_dir);
            // Rewrite the hook files so they always match this version's
            // format and script locations.
            if let Err(e) = commands::sync_shell(&store) {
                eprintln!("Other: couldn't refresh shell files: {e}");
            }
            app.manage(AppState {
                store: Mutex::new(store),
                license: Mutex::new(license),
            });
            commands::revalidate_license_in_background(app.handle().clone());
            // macOS: a monochrome template icon the system recolors for light
            // and dark menu bars. Elsewhere the colored app icon, since a
            // black icon disappears on dark taskbars and panels.
            #[cfg(target_os = "macos")]
            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;
            #[cfg(not(target_os = "macos"))]
            let icon = app.default_window_icon().cloned().ok_or("missing app icon")?;
            tauri::tray::TrayIconBuilder::with_id(tray::TRAY_ID)
                .icon(icon)
                .icon_as_template(cfg!(target_os = "macos"))
                .tooltip("Other — Claude Code accounts")
                .menu(&tray::build_menu(app.handle())?)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| tray::on_menu_event(app, event.id().as_ref()))
                .build(app)?;
            // Signing in happens inside Claude Code, outside our view — keep
            // the menu's "signed in as" labels current.
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(20));
                let h = handle.clone();
                let _ = handle.run_on_main_thread(move || tray::refresh(&h));
            });
            Ok(())
        })
        // Closing the window keeps Other in the menu bar (and off the
        // Dock); the tray's "Manage Accounts" brings it back.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
                #[cfg(target_os = "macos")]
                let _ = window
                    .app_handle()
                    .set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::platform_info,
            commands::list_accounts,
            commands::create_account,
            commands::delete_account,
            commands::open_claude_code,
            commands::open_claude_app,
            commands::copy_settings,
            commands::list_folders,
            commands::add_folder,
            commands::set_folder_account,
            commands::remove_folder,
            commands::open_folder,
            commands::install_shell_integration,
            commands::license_status,
            commands::activate_license,
            commands::deactivate_license,
            commands::buy_license
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
