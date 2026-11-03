//! The menu-bar menu: one submenu per account (labelled with who it's signed
//! in as) to open the Claude app, start Claude Code in Terminal, or start it
//! in one of the account's folders.

use crate::claude_app;
use crate::license::{self, Status};
use crate::commands::{account_views, open_claude_app_as, open_claude_code_as, AppState};
use std::path::Path;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, Wry};

pub const TRAY_ID: &str = "main";

pub fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let (accounts, folders, license) = {
        let state = app.state::<AppState>();
        let store = state.store.lock().unwrap();
        let license = state.license.lock().unwrap().status();
        (account_views(&store), store.folders(), license)
    };
    let has_app = claude_app::installed();
    let item = |id: String, label: &str| MenuItem::with_id(app, id, label, true, None::<&str>);

    let menu = Menu::new(app)?;
    for a in &accounts {
        let label = match &a.email {
            Some(email) => format!("{} — {email}", a.name),
            None => format!("{} — not signed in", a.name),
        };
        let sub = Submenu::with_id(app, format!("acct:{}", a.id), label, true)?;
        if has_app {
            sub.append(&item(format!("app:{}", a.id), "Open Claude App")?)?;
        }
        sub.append(&item(format!("code:{}", a.id), "Open Claude Code in Terminal")?)?;
        let owned: Vec<_> = folders.iter().filter(|f| f.account_id == a.id).collect();
        if !owned.is_empty() {
            sub.append(&PredefinedMenuItem::separator(app)?)?;
            for f in owned {
                sub.append(&item(format!("folder:{}", f.id), &format!("Start in {}", f.name))?)?;
            }
        }
        menu.append(&sub)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    // Until licensed: where the trial stands, and how to buy or enter a key.
    let trial_label = match &license {
        Status::Trial { days_left } => Some(format!("Trial: {days_left} day{} left", if *days_left == 1 { "" } else { "s" })),
        Status::Expired => Some("Trial ended".to_string()),
        Status::Free | Status::Licensed { .. } => None,
    };
    if let Some(label) = trial_label {
        menu.append(&MenuItem::with_id(app, "trial", label, false, None::<&str>)?)?;
        menu.append(&item("buy".into(), "Buy License…")?)?;
        menu.append(&item("enter-key".into(), "Enter License Key…")?)?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    menu.append(&item("open".into(), "Manage Accounts…")?)?;
    menu.append(&item("quit".into(), "Quit Other")?)?;
    Ok(menu)
}

/// Rebuild the menu, e.g. after accounts or folders change.
pub fn refresh(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_menu(app) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

pub fn show_main_window(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

pub fn on_menu_event(app: &AppHandle, id: &str) {
    match id {
        "open" => show_main_window(app),
        "buy" => {
            let _ = license::open_url(license::BUY_URL);
        }
        "enter-key" => {
            show_main_window(app);
            let _ = app.emit("enter-license-key", ());
        }
        "quit" => app.exit(0),
        other => {
            let state = app.state::<AppState>();
            let store = state.store.lock().unwrap();
            let result = if let Some(account) = other.strip_prefix("app:") {
                open_claude_app_as(&store, account)
            } else if let Some(account) = other.strip_prefix("code:") {
                open_claude_code_as(&store, account, None)
            } else if let Some(folder) = other.strip_prefix("folder:") {
                match store.get_folder(folder) {
                    Some(f) => open_claude_code_as(&store, &f.account_id, Some(Path::new(&f.path))),
                    None => Ok(()),
                }
            } else {
                Ok(())
            };
            if let Err(e) = result {
                eprintln!("Other: {e}");
            }
        }
    }
}
