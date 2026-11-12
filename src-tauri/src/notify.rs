//! System notifications from the launch wrapper (`checkpoint run …`), which has no window: what it
//! restored or backed up, and when something was left for the app to decide.
//!
//! - Windows: a toast. Installed builds use the app's ID (the NSIS installer registers it on the
//!   Start menu shortcut); debug builds have no registered ID and use notify-rust's default.
//! - Linux: freedesktop notifications over D-Bus. Whether Steam's Gaming Mode shows them depends
//!   on the session; in desktop mode they show like any other app's.
//!
//! A notification that cannot be shown is only logged: it never stops the game or the sync.

use crate::config::Config;
use log::warn;

/// `identifier` in tauri.conf.json
#[cfg(windows)]
const APP_ID: &str = "dev.checkpoint.savesync";

pub fn show(cfg: &Config, title: &str, body: &str) {
    if !cfg.launch_notifications {
        return;
    }
    let mut n = notify_rust::Notification::new();
    n.summary(title).body(body).appname("Checkpoint");
    #[cfg(windows)]
    if !cfg!(debug_assertions) {
        n.app_id(APP_ID);
    }
    #[cfg(target_os = "linux")]
    n.timeout(notify_rust::Timeout::Milliseconds(6000));
    if let Err(e) = n.show() {
        warn!("Launch wrapper: notification not shown: {e}");
    }
}

#[cfg(test)]
mod tests {
    /// Shows a real notification: `cargo test notify_real -- --ignored`
    #[test]
    #[ignore]
    fn notify_real() {
        let mut cfg = crate::config::Config::load();
        cfg.launch_notifications = true;
        super::show(&cfg, "Checkpoint", "Test notification from the launch wrapper.");
    }
}
