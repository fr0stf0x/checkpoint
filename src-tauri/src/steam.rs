//! Đọc thông tin Steam từ đĩa: thư mục cài, thư viện, game đã cài, tài khoản.
//! Tất cả đều là file local nên chạy được khi Steam ở offline mode.

use crate::vdf;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const STEAMID64_BASE: u64 = 76561197960265728;

pub fn find_root(override_path: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = override_path.filter(|p| !p.trim().is_empty()) {
        let p = PathBuf::from(p);
        return p.join("steamapps").exists().then_some(p);
    }
    candidates().into_iter().find(|p| p.join("steamapps").exists())
}

#[cfg(windows)]
fn candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    use winreg::enums::HKEY_CURRENT_USER;
    if let Ok(key) = winreg::RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam") {
        if let Ok(p) = key.get_value::<String, _>("SteamPath") {
            v.push(PathBuf::from(p.replace('/', "\\")));
        }
    }
    v.push(PathBuf::from("C:\\Program Files (x86)\\Steam"));
    v
}

#[cfg(not(windows))]
fn candidates() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else { return vec![] };
    vec![
        home.join(".local/share/Steam"),
        home.join(".steam/steam"),
        home.join(".steam/root"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    ]
}

pub fn libraries(root: &Path) -> Vec<PathBuf> {
    let mut libs = vec![root.to_path_buf()];
    if let Ok(text) = fs::read_to_string(root.join("steamapps").join("libraryfolders.vdf")) {
        let v = vdf::parse(&text);
        if let Some(lf) = v.get("libraryfolders") {
            for (_, entry) in lf.entries() {
                if let Some(p) = entry.str("path") {
                    libs.push(PathBuf::from(p));
                }
            }
        }
    }
    let mut out: Vec<PathBuf> = Vec::new();
    for l in libs {
        let key = norm(&l);
        if l.join("steamapps").exists() && !out.iter().any(|o| norm(o) == key) {
            out.push(l);
        }
    }
    out
}

fn norm(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_string();
    if cfg!(windows) { s.to_lowercase() } else { s }
}

#[derive(Debug, Clone, Serialize)]
pub struct InstalledGame {
    pub appid: u32,
    pub name: String,
    pub installdir: String,
    pub library: PathBuf,
}

/// Các app không phải game (runtime, Proton, redistributable).
fn is_tool(appid: u32, name: &str) -> bool {
    const TOOL_IDS: &[u32] = &[228980, 1070560, 1391110, 1628350, 1493710, 2180100];
    let n = name.to_lowercase();
    TOOL_IDS.contains(&appid)
        || n.starts_with("proton")
        || n.contains("steam linux runtime")
        || n.contains("steamworks common")
        || n.contains("steamvr")
}

pub fn installed_games(libs: &[PathBuf]) -> Vec<InstalledGame> {
    let mut out = Vec::new();
    for lib in libs {
        let Ok(rd) = fs::read_dir(lib.join("steamapps")) else { continue };
        for e in rd.flatten() {
            let fname = e.file_name().to_string_lossy().to_string();
            if !(fname.starts_with("appmanifest_") && fname.ends_with(".acf")) {
                continue;
            }
            let Ok(text) = fs::read_to_string(e.path()) else { continue };
            let v = vdf::parse(&text);
            let Some(st) = v.get("AppState") else { continue };
            let Some(appid) = st.str("appid").and_then(|s| s.parse().ok()) else { continue };
            let name = st.str("name").unwrap_or("").to_string();
            let installdir = st.str("installdir").unwrap_or("").to_string();
            if is_tool(appid, &name) || installdir.is_empty() {
                continue;
            }
            if !out.iter().any(|g: &InstalledGame| g.appid == appid) {
                out.push(InstalledGame { appid, name, installdir, library: lib.clone() });
            }
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

#[derive(Debug, Clone, Serialize)]
pub struct Account {
    pub steamid64: String,
    pub account_id: u64,
    pub account_name: String,
    pub persona_name: String,
    pub most_recent: bool,
    pub offline_mode: bool,
    pub timestamp: i64,
    pub has_userdata: bool,
    pub userdata_apps: usize,
}

pub fn accounts(root: &Path) -> Vec<Account> {
    let Ok(text) = fs::read_to_string(root.join("config").join("loginusers.vdf")) else {
        return vec![];
    };
    let v = vdf::parse(&text);
    let Some(users) = v.get("users") else { return vec![] };
    let mut out = Vec::new();
    for (sid, u) in users.entries() {
        let Ok(sid64) = sid.parse::<u64>() else { continue };
        let account_id = sid64.saturating_sub(STEAMID64_BASE);
        let ud = root.join("userdata").join(account_id.to_string());
        let userdata_apps = fs::read_dir(&ud)
            .map(|rd| rd.flatten().filter(|e| e.file_name().to_string_lossy().parse::<u32>().is_ok()).count())
            .unwrap_or(0);
        out.push(Account {
            steamid64: sid.clone(),
            account_id,
            account_name: u.str("AccountName").unwrap_or("").to_string(),
            persona_name: u.str("PersonaName").unwrap_or("").to_string(),
            most_recent: u.str("MostRecent") == Some("1"),
            offline_mode: u.str("WantsOfflineMode") == Some("1"),
            timestamp: u.str("timestamp").and_then(|t| t.parse().ok()).unwrap_or(0),
            has_userdata: ud.exists(),
            userdata_apps,
        });
    }
    // Không có MostRecent thì coi account đăng nhập gần nhất (timestamp lớn nhất) là hiện tại
    if !out.iter().any(|a| a.most_recent) {
        if let Some(a) = out.iter_mut().max_by_key(|a| a.timestamp) {
            a.most_recent = true;
        }
    }
    out.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    out
}
