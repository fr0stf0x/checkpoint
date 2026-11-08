//! Builds a prefilled GitHub "New issue" link.
//!
//! No GitHub token in the app: the link opens in the browser, the user signs in to GitHub
//! and presses Submit. Issue form fields (.github/ISSUE_TEMPLATE/*.yml) are prefilled through
//! URL parameters named after each field's `id`. URLs are limited to about 8 KB, so the log
//! is trimmed from the start to fit. The prefilled text is English for the public tracker.
//! A report about one game only takes that game's log lines (plus warnings and errors), so the
//! part that matters fits in the URL.

use crate::logger;

pub const REPO: &str = "fr0stf0x/checkpoint";
/// Keep the URL below what GitHub accepts (~8 KB), with a safety margin.
const MAX_URL: usize = 7600;

fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn system_line() -> String {
    let os = sysinfo::System::long_os_version().unwrap_or_else(|| std::env::consts::OS.to_string());
    format!("{os} · {}", std::env::consts::ARCH)
}

pub struct IssueLink {
    pub url: String,
    pub log_truncated: bool,
}

/// What the UI knows about the game a report is about (English, no user-facing translation).
pub struct GameContext {
    pub appid: u32,
    pub name: String,
    /// e.g. status, save folders, cloud versions; home folder and SteamIDs are masked here
    pub details: String,
}

pub fn issue_url(kind: &str, title: &str, error: Option<&str>, game: Option<&GameContext>) -> IssueLink {
    let base = format!("https://github.com/{REPO}/issues/new");
    if kind == "feature" {
        return IssueLink {
            url: format!("{base}?template=feature_request.yml&title={}", enc(title)),
            log_truncated: false,
        };
    }
    let mut what = match error {
        Some(e) if !e.trim().is_empty() => format!("Error shown in the app:\n\n```\n{}\n```\n\n", logger::redact_ids(e.trim())),
        _ => String::new(),
    };
    if let Some(g) = game {
        // Capped so the log still gets most of the URL
        let details: String = logger::redact_ids(&logger::redact_home(g.details.trim())).chars().take(1500).collect();
        what.push_str(&format!("Game as Checkpoint sees it:\n\n```\n{details}\n```\n\nWhat went wrong: \n"));
    }
    let mut head = format!(
        "{base}?template=bug_report.yml&title={}&version={}&os={}&what-happened={}",
        enc(title),
        enc(env!("CARGO_PKG_VERSION")),
        enc(&system_line()),
        enc(&what)
    );
    if let Some(g) = game {
        head.push_str(&format!("&game={}", enc(&format!("{} (AppID {})", g.name, g.appid))));
    }
    let budget = MAX_URL.saturating_sub(head.len() + "&logs=".len());
    let all = match game {
        Some(g) => logger::tail_for_app(g.appid, 300),
        None => logger::tail(300),
    };
    let mut lines: Vec<&str> = all.lines().collect();
    let total = lines.len();
    let mut encoded = enc(&lines.join("\n"));
    while encoded.len() > budget && !lines.is_empty() {
        let drop = (lines.len() / 10).max(1);
        lines.drain(..drop);
        encoded = enc(&lines.join("\n"));
    }
    IssueLink { url: format!("{head}&logs={encoded}"), log_truncated: lines.len() < total }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_fits() {
        assert_eq!(enc("a b&c=é"), "a%20b%26c%3D%C3%A9");
        let l = issue_url("bug", "[Bug] Backup failed", Some("rclone failed: 76561199698261377"), None);
        assert!(l.url.len() <= MAX_URL + 10, "url too long: {}", l.url.len());
        assert!(l.url.contains("template=bug_report.yml"));
        assert!(!l.url.contains("76561199698261377"), "SteamID must be masked");
        let f = issue_url("feature", "Idea", None, None);
        assert!(f.url.contains("template=feature_request.yml") && !f.url.contains("logs="));
        let g = GameContext { appid: 4115450, name: "Phantom Blade Zero".into(), details: "Saved/76561199698261377/x.sav".into() };
        let l = issue_url("bug", "[Bug] Phantom Blade Zero: ", None, Some(&g));
        assert!(l.url.contains("&game=Phantom%20Blade%20Zero%20%28AppID%204115450%29"));
        assert!(!l.url.contains("76561199698261377"), "SteamID in details must be masked");
        assert!(l.url.len() <= MAX_URL + 10);
    }
}
