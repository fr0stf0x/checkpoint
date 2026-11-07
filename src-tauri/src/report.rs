//! Tạo link "New issue" trên GitHub với nội dung điền sẵn.
//!
//! Không cần token GitHub trong app: link mở trong trình duyệt, người dùng tự đăng nhập GitHub
//! và bấm Submit. Các ô của issue form (.github/ISSUE_TEMPLATE/*.yml) được điền qua tham số URL
//! theo `id` của từng ô. URL giới hạn khoảng 8 KB nên log được cắt bớt từ đầu cho vừa.

use crate::logger;

pub const REPO: &str = "fr0stf0x/checkpoint";
/// Giữ URL dưới ngưỡng GitHub chấp nhận (~8 KB), chừa khoảng an toàn.
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

pub fn issue_url(kind: &str, title: &str, error: Option<&str>) -> IssueLink {
    let base = format!("https://github.com/{REPO}/issues/new");
    if kind == "feature" {
        return IssueLink {
            url: format!("{base}?template=feature_request.yml&title={}", enc(title)),
            log_truncated: false,
        };
    }
    let what = match error {
        Some(e) if !e.trim().is_empty() => format!("Thông báo lỗi trong app / Error shown in the app:\n\n```\n{}\n```\n\n", logger::redact_ids(e.trim())),
        _ => String::new(),
    };
    let head = format!(
        "{base}?template=bug_report.yml&title={}&version={}&os={}&what-happened={}",
        enc(title),
        enc(env!("CARGO_PKG_VERSION")),
        enc(&system_line()),
        enc(&what)
    );
    let budget = MAX_URL.saturating_sub(head.len() + "&logs=".len());
    let all = logger::tail(300);
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
        assert_eq!(enc("a b&c=đ"), "a%20b%26c%3D%C4%91");
        let l = issue_url("bug", "[Lỗi] Backup thất bại", Some("rclone lỗi: 76561199698261377"));
        assert!(l.url.len() <= MAX_URL + 10, "url dài {}", l.url.len());
        assert!(l.url.contains("template=bug_report.yml"));
        assert!(!l.url.contains("76561199698261377"), "SteamID phải được che");
        let f = issue_url("feature", "Ý tưởng", None);
        assert!(f.url.contains("template=feature_request.yml") && !f.url.contains("logs="));
    }
}
