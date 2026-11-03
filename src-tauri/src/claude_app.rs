//! The Claude desktop app as a separate instance per account, side by side
//! with the others. Each instance gets its own `--user-data-dir` (its own web
//! login) and `CLAUDE_CONFIG_DIR` (so its built-in Claude Code uses the
//! account's config too). There's no Claude desktop app for Linux.

use crate::claude::CONFIG_ENV;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The app's main executable, if installed.
#[cfg(target_os = "macos")]
fn executable() -> Option<PathBuf> {
    let exe = PathBuf::from("/Applications/Claude.app/Contents/MacOS/Claude");
    exe.is_file().then_some(exe)
}

/// The Squirrel launcher in the per-user install. It forwards arguments to
/// the current version. (Untested on Windows — verify before release.)
#[cfg(windows)]
fn executable() -> Option<PathBuf> {
    let exe = PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
        .join("AnthropicClaude")
        .join("claude.exe");
    exe.is_file().then_some(exe)
}

#[cfg(target_os = "linux")]
fn executable() -> Option<PathBuf> {
    None
}

pub fn installed() -> bool {
    executable().is_some()
}

/// The pid of the running instance for an account, if any: the main process
/// launched with that `--user-data-dir`, or — for the default account
/// (`user_data_dir` None) — the one launched without the flag.
#[cfg(target_os = "macos")]
fn pid_for(user_data_dir: Option<&Path>) -> Option<i32> {
    let exe = executable()?;
    let exe = exe.to_string_lossy();
    let out = Command::new("/bin/ps").args(["-axo", "pid=,args="]).output().ok()?;
    let wanted = user_data_dir.map(|d| format!("--user-data-dir={}", d.to_string_lossy()));
    String::from_utf8_lossy(&out.stdout).lines().find_map(|line| {
        let (pid, args) = line.trim_start().split_once(' ')?;
        let rest = args.strip_prefix(exe.as_ref())?;
        // Exactly the main executable, not "Claude Helper…" processes.
        if !(rest.is_empty() || rest.starts_with(' ')) {
            return None;
        }
        let matches = match &wanted {
            Some(flag) => rest.contains(flag.as_str()),
            None => !rest.contains("--user-data-dir"),
        };
        matches.then(|| pid.parse().ok()).flatten()
    })
}

/// Whether the account's instance is running. Only detectable on macOS;
/// elsewhere this answers false.
pub fn is_running(user_data_dir: Option<&Path>) -> bool {
    #[cfg(target_os = "macos")]
    return pid_for(user_data_dir).is_some();
    #[cfg(not(target_os = "macos"))]
    {
        let _ = user_data_dir;
        false
    }
}

/// Bring a process's windows to the front. Needed on macOS because every
/// instance opens its window at the same spot, so a new account's window
/// would otherwise sit hidden behind the one already in front.
#[cfg(target_os = "macos")]
fn activate_pid(pid: i32) -> bool {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
    match NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
        Some(app) => app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows),
        None => false,
    }
}

/// Open the Claude desktop app as an account, alongside any other account's
/// instance. `config_dir` None = the default account. If the account's
/// instance is already running it's brought to the front instead.
pub fn open(config_dir: Option<&Path>, user_data_dir: Option<&Path>) -> Result<(), String> {
    let exe = executable().ok_or("The Claude app isn't installed on this computer")?;

    #[cfg(target_os = "macos")]
    if let Some(pid) = pid_for(user_data_dir) {
        activate_pid(pid);
        return Ok(());
    }

    let mut args = Vec::new();
    if let Some(dir) = user_data_dir {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        args.push(format!("--user-data-dir={}", dir.to_string_lossy()));
    }
    let mut cmd = detached_command(&exe, &args);
    match config_dir {
        Some(dir) => cmd.env(CONFIG_ENV, dir),
        None => cmd.env_remove(CONFIG_ENV),
    };
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    let user_data_dir = user_data_dir.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let _ = child.wait();
        // macOS: raise the new instance once it's up. Its window appears a
        // moment after the process, so keep activating for a couple of seconds.
        #[cfg(target_os = "macos")]
        {
            let mut raised = 0;
            for _ in 0..40 {
                std::thread::sleep(std::time::Duration::from_millis(250));
                if let Some(pid) = pid_for(user_data_dir.as_deref()) {
                    activate_pid(pid);
                    raised += 1;
                    if raised >= 8 {
                        break;
                    }
                }
            }
        }
    });
    Ok(())
}

/// A command that starts `exe` so it outlives Other. On macOS it's spawned
/// directly (LaunchServices would just re-activate whichever instance is
/// already running) through an `sh` that exits immediately.
#[cfg(unix)]
fn detached_command(exe: &Path, args: &[String]) -> Command {
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(r#""$0" "$@" >/dev/null 2>&1 &"#)
        .arg(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd
}

#[cfg(windows)]
fn detached_command(exe: &Path, args: &[String]) -> Command {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    cmd
}
