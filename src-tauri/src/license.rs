//! Licensing for official builds: a 7-day trial, then a gentle reminder to
//! buy a license. Nothing is ever locked. Builds made with the `free` Cargo
//! feature (`npm run build:free`) skip all of this.
//!
//! License keys are checked against a small licensing API (see LICENSING.md
//! for the contract). The app only sends the key, a random per-install
//! device id and the computer's name; no secrets are built into the app.
//! State lives in `license.json` in the app's data folder.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Product id sent to the licensing API (one API serves several apps).
const PRODUCT: &str = "other";
/// Licensing API base URL. Overridable at build time for testing.
const API_BASE: &str = match option_env!("OTHER_LICENSE_API") {
    Some(url) => url,
    None => "https://licensing.xvelopers.com",
};
/// Where "Buy License" goes.
pub const BUY_URL: &str = match option_env!("OTHER_BUY_URL") {
    Some(url) => url,
    None => "https://ai.xvelopers.com/other",
};
const TRIAL_DAYS: u64 = 7;
const DAY: u64 = 24 * 60 * 60;
/// How often a stored license is re-checked (e.g. for refunds).
const REVALIDATE_EVERY: u64 = 7 * DAY;

/// What the window and menu bar show.
#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Status {
    /// Built with the `free` feature: no licensing at all.
    Free,
    Trial { days_left: u64 },
    Expired,
    Licensed { email: Option<String> },
}

#[derive(Serialize, Deserialize, Default)]
struct LicenseFile {
    /// When the trial started (unix seconds).
    trial_started: Option<u64>,
    /// Random id for this install, sent with activations.
    device_id: Option<String>,
    key: Option<String>,
    activation_id: Option<String>,
    email: Option<String>,
    /// Last time the API confirmed the license (unix seconds).
    last_validated: Option<u64>,
}

pub struct Licensing {
    path: PathBuf,
    file: LicenseFile,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Licensing {
    /// Load the license state, starting the trial on first launch.
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("license.json");
        let file = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let mut me = Self { path, file };
        if cfg!(not(feature = "free")) {
            let mut changed = false;
            if me.file.trial_started.is_none() {
                me.file.trial_started = Some(now());
                changed = true;
            }
            if me.file.device_id.is_none() {
                me.file.device_id = Some(uuid::Uuid::new_v4().to_string());
                changed = true;
            }
            if changed {
                let _ = me.save();
            }
        }
        me
    }

    fn save(&self) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(&self.file).map_err(|e| e.to_string())?;
        fs::write(&self.path, json).map_err(|e| e.to_string())
    }

    pub fn status(&self) -> Status {
        if cfg!(feature = "free") {
            return Status::Free;
        }
        if self.file.key.is_some() && self.file.activation_id.is_some() {
            return Status::Licensed { email: self.file.email.clone() };
        }
        let started = self.file.trial_started.unwrap_or_else(now);
        let days_used = now().saturating_sub(started) / DAY;
        if days_used >= TRIAL_DAYS {
            Status::Expired
        } else {
            Status::Trial { days_left: TRIAL_DAYS - days_used }
        }
    }

    /// What an activation request needs, so the network call can run
    /// without holding the app's lock.
    pub fn device(&self) -> (String, String) {
        (self.file.device_id.clone().unwrap_or_default(), device_name())
    }

    /// The stored key and activation, if licensed.
    pub fn activation(&self) -> Option<(String, String)> {
        Some((self.file.key.clone()?, self.file.activation_id.clone()?))
    }

    pub fn revalidation_due(&self) -> bool {
        self.activation().is_some()
            && now().saturating_sub(self.file.last_validated.unwrap_or(0)) >= REVALIDATE_EVERY
    }

    pub fn set_licensed(&mut self, key: String, activated: Activated) -> Result<(), String> {
        self.file.key = Some(key);
        self.file.activation_id = Some(activated.activation_id);
        self.file.email = activated.email;
        self.file.last_validated = Some(now());
        self.save()
    }

    pub fn mark_validated(&mut self, email: Option<String>) -> Result<(), String> {
        if email.is_some() {
            self.file.email = email;
        }
        self.file.last_validated = Some(now());
        self.save()
    }

    /// Forget the license on this computer. The trial doesn't restart.
    pub fn clear_license(&mut self) -> Result<(), String> {
        self.file.key = None;
        self.file.activation_id = None;
        self.file.email = None;
        self.file.last_validated = None;
        self.save()
    }
}

/// The computer's name, shown in the buyer's list of activated devices.
fn device_name() -> String {
    #[cfg(target_os = "macos")]
    let name = std::process::Command::new("/usr/sbin/scutil")
        .args(["--get", "ComputerName"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    #[cfg(windows)]
    let name = std::env::var("COMPUTERNAME").ok();
    #[cfg(target_os = "linux")]
    let name = fs::read_to_string("/etc/hostname").ok().map(|s| s.trim().to_string());
    name.filter(|n| !n.is_empty()).unwrap_or_else(|| "Unknown computer".into())
}

// ---------- Licensing API ----------

#[derive(Deserialize)]
pub struct Activated {
    pub activation_id: String,
    pub email: Option<String>,
}

#[derive(Deserialize)]
struct Validated {
    valid: bool,
    email: Option<String>,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .into()
}

fn post(endpoint: &str, body: serde_json::Value) -> Result<ureq::http::Response<ureq::Body>, String> {
    agent()
        .post(format!("{API_BASE}/v1/licenses/{endpoint}"))
        .send_json(body)
        .map_err(|_| "Couldn't reach the license server. Check your internet connection and try again.".into())
}

/// Activate a key on this computer.
pub fn activate(key: &str, device_id: &str, device_name: &str) -> Result<Activated, String> {
    let mut resp = post(
        "activate",
        serde_json::json!({ "product": PRODUCT, "key": key, "device_id": device_id, "device_name": device_name }),
    )?;
    match resp.status().as_u16() {
        200 => resp.body_mut().read_json::<Activated>().map_err(|e| e.to_string()),
        404 => Err("That license key wasn't found. Check it and try again.".into()),
        403 => Err("This key is already in use on its maximum number of computers. Deactivate it on another computer first.".into()),
        410 => Err("This license is no longer valid (refunded or revoked).".into()),
        code => Err(format!("The license server returned an error ({code}). Please try again later.")),
    }
}

/// Re-check a stored license. Ok(None) = no longer valid; Err = couldn't
/// check (e.g. offline), so the license is kept.
pub fn validate(key: &str, activation_id: &str) -> Result<Option<Option<String>>, String> {
    let mut resp = post(
        "validate",
        serde_json::json!({ "product": PRODUCT, "key": key, "activation_id": activation_id }),
    )?;
    match resp.status().as_u16() {
        200 => {
            let v = resp.body_mut().read_json::<Validated>().map_err(|e| e.to_string())?;
            Ok(v.valid.then_some(v.email))
        }
        404 | 410 => Ok(None),
        code => Err(format!("license server error {code}")),
    }
}

/// Free this computer's activation so the key can be used elsewhere.
pub fn deactivate(key: &str, activation_id: &str) -> Result<(), String> {
    let resp = post(
        "deactivate",
        serde_json::json!({ "product": PRODUCT, "key": key, "activation_id": activation_id }),
    )?;
    match resp.status().as_u16() {
        // 404: already gone on the server — fine.
        200 | 204 | 404 => Ok(()),
        code => Err(format!("The license server returned an error ({code}). Please try again later.")),
    }
}

/// Open a URL in the default browser.
pub fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("/usr/bin/open");
    #[cfg(target_os = "linux")]
    let mut cmd = std::process::Command::new("xdg-open");
    #[cfg(windows)]
    let mut cmd = {
        use std::os::windows::process::CommandExt;
        let mut c = std::process::Command::new("rundll32.exe");
        c.arg("url.dll,FileProtocolHandler").creation_flags(0x0800_0000);
        c
    };
    cmd.arg(url).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[cfg(not(feature = "free"))]
    #[test]
    fn trial_counts_down_then_expires_and_a_license_ends_it() {
        let dir = sandbox("other-license-test");
        let mut lic = Licensing::load(&dir);
        assert_eq!(lic.status(), Status::Trial { days_left: 7 });
        assert!(dir.join("license.json").is_file(), "trial start is saved on first launch");

        lic.file.trial_started = Some(now() - 3 * DAY - 60);
        assert_eq!(lic.status(), Status::Trial { days_left: 4 });
        lic.file.trial_started = Some(now() - 8 * DAY);
        assert_eq!(lic.status(), Status::Expired);

        lic.set_licensed(
            "KEY".into(),
            Activated { activation_id: "act".into(), email: Some("me@x.com".into()) },
        )
        .unwrap();
        assert_eq!(lic.status(), Status::Licensed { email: Some("me@x.com".into()) });
        assert!(!lic.revalidation_due());

        // Survives a restart; clearing it goes back to the (expired) trial.
        let mut lic = Licensing::load(&dir);
        assert!(matches!(lic.status(), Status::Licensed { .. }));
        lic.clear_license().unwrap();
        assert_eq!(lic.status(), Status::Expired);
    }

    #[cfg(feature = "free")]
    #[test]
    fn free_builds_are_never_asked() {
        let dir = sandbox("other-license-free-test");
        assert_eq!(Licensing::load(&dir).status(), Status::Free);
    }
}
