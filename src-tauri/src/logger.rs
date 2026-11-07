//! Ghi log ra file trên máy người dùng để dễ báo lỗi.
//!
//! - File: `<data>/logs/checkpoint.log`, quá 2 MB thì đổi thành `checkpoint.1.log` (giữ 1 bản cũ).
//! - Đường dẫn thư mục home được thay bằng `~` ngay khi ghi.
//! - Khi lấy log để gửi báo cáo, SteamID64 cũng được che bớt.
//! - Không bao giờ ghi token, client secret hay tên tài khoản Steam vào log.

use crate::config::data_dir;
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

const MAX_BYTES: u64 = 2 * 1024 * 1024;

pub fn log_dir() -> PathBuf {
    data_dir().join("logs")
}

pub fn log_path() -> PathBuf {
    log_dir().join("checkpoint.log")
}

fn old_log_path() -> PathBuf {
    log_dir().join("checkpoint.1.log")
}

struct FileLogger {
    file: Mutex<Option<File>>,
    homes: Vec<String>,
}

impl FileLogger {
    fn open() -> Option<File> {
        fs::create_dir_all(log_dir()).ok()?;
        OpenOptions::new().create(true).append(true).open(log_path()).ok()
    }

    fn redact(&self, s: &str) -> String {
        let mut out = s.to_string();
        for h in &self.homes {
            out = out.replace(h.as_str(), "~");
        }
        out
    }
}

impl Log for FileLogger {
    fn enabled(&self, m: &Metadata) -> bool {
        // Log của app từ mức Info, của thư viện chỉ từ Warn
        if m.target().starts_with("checkpoint") { m.level() <= Level::Info } else { m.level() <= Level::Warn }
    }

    fn log(&self, r: &Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let line = format!(
            "{} {:<5} [{}] {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f%:z"),
            r.level(),
            r.target().trim_start_matches("checkpoint::"),
            self.redact(&r.args().to_string())
        );
        if cfg!(debug_assertions) {
            eprint!("{line}");
        }
        let mut g = self.file.lock().unwrap_or_else(|e| e.into_inner());
        let too_big = fs::metadata(log_path()).map(|m| m.len() > MAX_BYTES).unwrap_or(false);
        if too_big {
            *g = None;
            let _ = fs::rename(log_path(), old_log_path());
        }
        if g.is_none() {
            *g = Self::open();
        }
        if let Some(f) = g.as_mut() {
            let _ = f.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {
        if let Some(f) = self.file.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            let _ = f.flush();
        }
    }
}

pub fn init() {
    let mut homes = Vec::new();
    if let Some(h) = dirs::home_dir() {
        let s = h.to_string_lossy().to_string();
        if s.len() > 3 {
            homes.push(s.replace('\\', "/"));
            homes.push(s);
        }
    }
    let logger = FileLogger { file: Mutex::new(FileLogger::open()), homes };
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
}

/// Che SteamID64 (17 chữ số bắt đầu bằng 7656), chỉ giữ 3 số cuối để vẫn phân biệt được.
pub fn redact_ids(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        let is_id = i + 17 <= b.len()
            && b[i..i + 17].iter().all(|c| c.is_ascii_digit())
            && &b[i..i + 4] == b"7656"
            && (i == 0 || !b[i - 1].is_ascii_digit())
            && (i + 17 == b.len() || !b[i + 17].is_ascii_digit());
        if is_id {
            out.push_str("7656…");
            out.push_str(&s[i + 14..i + 17]);
            i += 17;
        } else {
            let ch = s[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// `n` dòng log cuối (gồm cả file cũ nếu cần), đã che SteamID.
pub fn tail(n: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    for p in [old_log_path(), log_path()] {
        if let Ok(t) = fs::read_to_string(&p) {
            lines.extend(t.lines().map(str::to_string));
        }
    }
    let start = lines.len().saturating_sub(n);
    redact_ids(&lines[start..].join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_steam_ids_only() {
        assert_eq!(redact_ids("Steam/76561199698261377/x.sav"), "Steam/7656…377/x.sav");
        assert_eq!(redact_ids("appid 4115450, 12 file"), "appid 4115450, 12 file");
        assert_eq!(redact_ids("176561199698261377"), "176561199698261377");
        assert_eq!(redact_ids("đã backup 76561198000000101."), "đã backup 7656…101.");
    }
}
