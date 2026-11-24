//! System notifications from the launch wrapper (`checkpoint run …`), which has no window: what it
//! restored or backed up, and when something was left for the app to decide.
//!
//! - Windows: a toast. Installed builds use the app's ID (the NSIS installer registers it on the
//!   Start menu shortcut); debug builds have no registered ID and use notify-rust's default.
//! - Linux: freedesktop notifications over D-Bus. Whether Steam's Gaming Mode shows them depends
//!   on the session; in desktop mode they show like any other app's. The result of a restore or
//!   backup replaces its "restoring…" / "backing up…" notification.
//!
//! A notification that cannot be shown is only logged: it never stops the game or the sync.

use crate::config::Config;
use log::warn;

/// `identifier` in tauri.conf.json
#[cfg(windows)]
const APP_ID: &str = "dev.checkpoint.savesync";

pub fn show(cfg: &Config, title: &str, body: &str) {
    if cfg.launch_notifications {
        show_always(title, body);
    }
}

/// Also when notifications are off: the only sign that the launch option did not start the game.
pub fn show_always(title: &str, body: &str) {
    if let Err(e) = build(title, body).show() {
        warn!("Launch wrapper: notification not shown: {e}");
    }
}

fn build(title: &str, body: &str) -> notify_rust::Notification {
    let mut n = notify_rust::Notification::new();
    n.summary(title).body(body).appname("Checkpoint");
    #[cfg(windows)]
    if !cfg!(debug_assertions) {
        n.app_id(APP_ID);
    }
    #[cfg(target_os = "linux")]
    n.timeout(notify_rust::Timeout::Milliseconds(6000));
    n
}

/// A "restoring…" / "backing up…" notification, followed by its result through [`Progress::finish`].
/// On Linux the result replaces it (same notification ID); Windows toasts cannot be replaced
/// through notify-rust, so the result is a second toast.
pub struct Progress {
    #[cfg(target_os = "linux")]
    handle: Option<notify_rust::NotificationHandle>,
}

pub fn start(cfg: &Config, title: &str, body: &str) -> Progress {
    let shown = if cfg.launch_notifications {
        build(title, body).show().map_err(|e| warn!("Launch wrapper: notification not shown: {e}")).ok()
    } else {
        None
    };
    #[cfg(target_os = "linux")]
    return Progress { handle: shown };
    #[cfg(not(target_os = "linux"))]
    {
        let _ = shown;
        Progress {}
    }
}

impl Progress {
    /// The result, when notifications are on.
    pub fn finish(self, cfg: &Config, title: &str, body: &str) {
        #[cfg(target_os = "linux")]
        if let Some(mut h) = self.handle {
            h.summary(title).body(body);
            match h.update() {
                Ok(()) => return,
                Err(e) => warn!("Launch wrapper: notification not updated: {e}"),
            }
        }
        show(cfg, title, body);
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
