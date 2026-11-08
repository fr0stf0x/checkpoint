//! File logging on the user's machine, to make bug reports useful.
//!
//! - File: `<data>/logs/checkpoint.log`. Past 2 MB it becomes `checkpoint.1.log` and older files
//!   shift to `.2` and `.3` (three old files kept, at most ~8 MB in all).
//! - Lines about one game carry `appid=<n>`, so a per-game bug report can pick them out.
//! - The home folder path is replaced with `~` when writing.
//! - When the log is exported for a report, SteamID64s are masked as well.
//! - Tokens, client secrets and Steam account names are never logged.
//! - Log messages are always in English, whatever the UI language.

use crate::config::data_dir;
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

const MAX_BYTES: u64 = 2 * 1024 * 1024;
/// Rotated files kept next to the current one.
const KEEP_OLD: usize = 3;

pub fn log_dir() -> PathBuf {
    data_dir().join("logs")
}

pub fn log_path() -> PathBuf {
    log_dir().join("checkpoint.log")
}

/// `checkpoint.<n>.log`, 1 = most recent rotated file.
fn old_log_path(n: usize) -> PathBuf {
    log_dir().join(format!("checkpoint.{n}.log"))
}

/// checkpoint.log → .1 → .2 → .3, dropping the oldest.
fn rotate() {
    let _ = fs::remove_file(old_log_path(KEEP_OLD));
    for n in (1..KEEP_OLD).rev() {
        let _ = fs::rename(old_log_path(n), old_log_path(n + 1));
    }
    let _ = fs::rename(log_path(), old_log_path(1));
}

/// Every log file, newest first.
fn files_newest_first() -> Vec<PathBuf> {
    let mut v = vec![log_path()];
    v.extend((1..=KEEP_OLD).map(old_log_path));
    v
}

/// Replace the home folder with `~` (also for text that did not go through the log, like save paths).
pub fn redact_home(s: &str) -> String {
    let mut out = s.to_string();
    for h in homes() {
        out = out.replace(h.as_str(), "~");
    }
    out
}

fn homes() -> Vec<String> {
    let mut homes = Vec::new();
    if let Some(h) = dirs::home_dir() {
        let s = h.to_string_lossy().to_string();
        if s.len() > 3 {
            homes.push(s.replace('\\', "/"));
            homes.push(s);
        }
    }
    homes
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
        // App logs from Info, third-party crates only from Warn
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
            rotate();
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
    let logger = FileLogger { file: Mutex::new(FileLogger::open()), homes: homes() };
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
}

/// Mask SteamID64s (17 digits starting with 7656), keeping the last 3 digits to tell them apart.
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

/// The last `n` log lines (reading rotated files only when needed), with SteamIDs masked.
pub fn tail(n: usize) -> String {
    let mut chunks: Vec<Vec<String>> = Vec::new();
    let mut have = 0;
    for p in files_newest_first() {
        if have >= n {
            break;
        }
        if let Ok(t) = fs::read_to_string(&p) {
            let lines: Vec<String> = t.lines().map(str::to_string).collect();
            have += lines.len();
            chunks.push(lines);
        }
    }
    let lines: Vec<String> = chunks.into_iter().rev().flatten().collect();
    let start = lines.len().saturating_sub(n);
    redact_ids(&lines[start..].join("\n"))
}

/// True when the line is about this game (`appid=<n>` not followed by another digit).
fn mentions_app(line: &str, appid: u32) -> bool {
    let tag = format!("appid={appid}");
    line.match_indices(&tag).any(|(i, _)| !line[i + tag.len()..].starts_with(|c: char| c.is_ascii_digit()))
}

/// What matters for a report about one game: its own lines, warnings and errors, and the
/// "starting" lines that separate app runs. The last `n` of those, SteamIDs masked.
pub fn tail_for_app(appid: u32, n: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    for p in files_newest_first().into_iter().rev() {
        if let Ok(t) = fs::read_to_string(&p) {
            lines.extend(t.lines().filter(|l| keep_for_app(l, appid)).map(str::to_string));
        }
    }
    let start = lines.len().saturating_sub(n);
    redact_ids(&lines[start..].join("\n"))
}

fn keep_for_app(l: &str, appid: u32) -> bool {
    // "<date> <time> <LEVEL> [target] message"
    let level = l.split_whitespace().nth(2).unwrap_or("");
    mentions_app(l, appid) || level == "ERROR" || level == "WARN" || (l.contains("] Checkpoint ") && l.contains(" starting "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_steam_ids_only() {
        assert_eq!(redact_ids("Steam/76561199698261377/x.sav"), "Steam/7656…377/x.sav");
        assert_eq!(redact_ids("appid 4115450, 12 file"), "appid 4115450, 12 file");
        assert_eq!(redact_ids("176561199698261377"), "176561199698261377");
        // multi-byte text around the ID must survive
        assert_eq!(redact_ids("café backup 76561198000000101."), "café backup 7656…101.");
    }

    #[test]
    fn picks_lines_of_one_game() {
        let l = |lvl: &str, msg: &str| format!("2026-11-08 10:00:00.000+07:00 {lvl:<5} [main] {msg}");
        assert!(keep_for_app(&l("INFO", "Backup appid=4115450 force=false"), 4115450));
        assert!(!keep_for_app(&l("INFO", "Backup appid=41154501 force=false"), 4115450));
        assert!(!keep_for_app(&l("INFO", "Backup appid=646570 force=false"), 4115450));
        assert!(keep_for_app(&l("ERROR", "list_games failed: x"), 4115450));
        assert!(keep_for_app(&l("WARN", "[ui] something"), 4115450));
        assert!(keep_for_app(&l("INFO", "Checkpoint 0.6.0 starting · Windows 11 · language=vi"), 4115450));
        assert!(!keep_for_app(&l("INFO", "Scanned 12 games"), 4115450));
    }
}
