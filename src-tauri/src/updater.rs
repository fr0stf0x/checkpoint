//! In-app updates through tauri-plugin-updater.
//!
//! The release workflow signs the installers and publishes `latest.json` on each GitHub Release;
//! the app reads it from `releases/latest/download/latest.json` (prereleases are skipped by GitHub).
//! Nothing is installed without the user clicking Update. The signature is checked by the plugin
//! against the public key in tauri.conf.json before anything is installed.

use crate::tr;
use anyhow::{anyhow, Result};
use log::{info, warn};
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

#[derive(Debug, Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub current: String,
    /// The "What's new" part of the release notes (Markdown as plain text)
    pub notes: String,
    /// false when this copy cannot replace itself (dev build, or Linux not run from an AppImage);
    /// the UI then links to the release page instead
    pub can_install: bool,
}

#[derive(Debug, Serialize, Default)]
pub struct Progress {
    pub downloaded: u64,
    /// 0 when the server did not send a size
    pub total: u64,
}

static DOWNLOADED: AtomicU64 = AtomicU64::new(0);
static TOTAL: AtomicU64 = AtomicU64::new(0);

pub fn progress() -> Progress {
    Progress { downloaded: DOWNLOADED.load(Ordering::Relaxed), total: TOTAL.load(Ordering::Relaxed) }
}

/// Whether this copy of the app can update itself in place.
pub fn can_self_update() -> bool {
    if cfg!(debug_assertions) {
        return false;
    }
    if cfg!(target_os = "linux") {
        // The updater replaces the file named by $APPIMAGE; a plain binary cannot be updated.
        return std::env::var_os("APPIMAGE").is_some();
    }
    true
}

fn updater(app: &AppHandle) -> Result<tauri_plugin_updater::Updater> {
    app.updater_builder()
        .on_before_exit(|| {
            info!("Closing for the update installer");
            log::logger().flush();
        })
        .build()
        .map_err(|e| anyhow!("{}", tr!("err.update_check", detail = e.to_string())))
}

/// Ask GitHub for a newer version. Runs on a blocking worker thread.
pub fn check(app: &AppHandle) -> Result<Option<UpdateInfo>> {
    let up = updater(app)?;
    let found = tauri::async_runtime::block_on(up.check())
        .map_err(|e| anyhow!("{}", tr!("err.update_check", detail = e.to_string())))?;
    Ok(found.map(|u| {
        info!("Update available: {} (current {})", u.version, u.current_version);
        UpdateInfo {
            version: u.version.clone(),
            current: u.current_version.clone(),
            notes: whats_new(u.body.as_deref().unwrap_or("")),
            can_install: can_self_update(),
        }
    }))
}

/// Download, verify and install the newest version, then restart.
/// On Windows the plugin starts the installer and exits the app itself.
/// Runs on a blocking worker thread; the caller holds the operation lock.
pub fn install(app: &AppHandle, expected: &str) -> Result<()> {
    if !can_self_update() {
        return Err(anyhow!("{}", tr!("err.update_unsupported")));
    }
    let up = updater(app)?;
    let update = tauri::async_runtime::block_on(up.check())
        .map_err(|e| anyhow!("{}", tr!("err.update_check", detail = e.to_string())))?
        .ok_or_else(|| anyhow!("{}", tr!("err.update_gone")))?;
    if update.version != expected {
        // A newer release appeared after the user read the notes: let them see the new ones first.
        return Err(anyhow!("{}", tr!("err.update_changed", version = update.version)));
    }

    DOWNLOADED.store(0, Ordering::Relaxed);
    TOTAL.store(0, Ordering::Relaxed);
    info!("Downloading update {}", update.version);
    let result = tauri::async_runtime::block_on(update.download_and_install(
        |chunk, total| {
            DOWNLOADED.fetch_add(chunk as u64, Ordering::Relaxed);
            if let Some(t) = total {
                TOTAL.store(t, Ordering::Relaxed);
            }
        },
        || info!("Update downloaded, signature verified, installing"),
    ));
    if let Err(e) = result {
        warn!("Update install failed after {} bytes", DOWNLOADED.load(Ordering::Relaxed));
        return Err(anyhow!("{}", tr!("err.update_install", detail = e.to_string())));
    }

    info!("Update {} installed, restarting", update.version);
    log::logger().flush();
    if cfg!(windows) {
        // Normally not reached: the plugin exits the app once the installer is running.
        return Ok(());
    }
    app.restart()
}

/// The "## What's new" section of the release body, or the whole body when it has no such heading.
pub fn whats_new(body: &str) -> String {
    let mut lines = body.lines();
    if lines.by_ref().any(|l| l.trim().eq_ignore_ascii_case("## what's new")) {
        let part: Vec<&str> = lines.take_while(|l| !l.starts_with("## ")).collect();
        return part.join("\n").trim().to_string();
    }
    body.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updater_pubkey_is_set() {
        let c: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let key = c["plugins"]["updater"]["pubkey"].as_str().unwrap_or("");
        assert!(key.len() > 40 && !key.contains("PLACEHOLDER"), "set the updater public key in tauri.conf.json");
    }

    #[test]
    fn whats_new_takes_only_that_section() {
        let body = "## What's new\n\n### Added\n- Copy link\n\n## Install\n\n**Windows:** run it.";
        assert_eq!(whats_new(body), "### Added\n- Copy link");
        assert_eq!(whats_new("  just text \n"), "just text");
        assert_eq!(whats_new(""), "");
        assert_eq!(whats_new("## What's new\n- a"), "- a");
    }
}
