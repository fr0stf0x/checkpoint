//! Biến các luật đường dẫn của manifest (`<winAppData>/Game/*.sav`) thành file thật trên máy,
//! và ngược lại: biến "đường dẫn logic" trong bản backup thành đường dẫn thật trên máy restore.
//!
//! Đường dẫn logic có dạng `<flavor>/<token>/<phần còn lại>`, ví dụ
//! `win/winAppData/EldenRing/76561198.../ER0000.sl2`. Nhờ vậy một bản backup từ Windows
//! restore được vào Proton prefix trên Bazzite và ngược lại.

use crate::manifest::FileRule;
use anyhow::{anyhow, bail, Result};
use glob::MatchOptions;
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flavor {
    /// Đường dẫn kiểu Windows: Windows thật, hoặc bên trong Proton prefix trên Linux
    Win,
    /// Đường dẫn Linux native
    Linux,
    /// Tương đối với thư mục Steam hoặc thư mục cài game
    Any,
}

impl Flavor {
    fn tag(self) -> &'static str {
        match self {
            Flavor::Win => "win",
            Flavor::Linux => "linux",
            Flavor::Any => "any",
        }
    }
    fn from_tag(s: &str) -> Option<Flavor> {
        match s {
            "win" => Some(Flavor::Win),
            "linux" => Some(Flavor::Linux),
            "any" => Some(Flavor::Any),
            _ => None,
        }
    }
}

pub struct GameCtx {
    pub appid: u32,
    pub installdir: String,
    pub base: PathBuf,
    pub steam_root: PathBuf,
    /// `<library>/steamapps/compatdata/<appid>/pfx/drive_c` (chỉ trên Linux)
    pub proton_drive_c: PathBuf,
}

impl GameCtx {
    pub fn new(appid: u32, installdir: &str, library: &Path, steam_root: &Path) -> Self {
        GameCtx {
            appid,
            installdir: installdir.to_string(),
            base: library.join("steamapps").join("common").join(installdir),
            steam_root: steam_root.to_path_buf(),
            proton_drive_c: library
                .join("steamapps")
                .join("compatdata")
                .join(appid.to_string())
                .join("pfx")
                .join("drive_c"),
        }
    }

    fn has_proton(&self) -> bool {
        cfg!(not(windows)) && self.proton_drive_c.exists()
    }
}

pub fn resolve(flavor: Flavor, token: &str, ctx: &GameCtx) -> Option<PathBuf> {
    match flavor {
        Flavor::Any => match token {
            "base" => Some(ctx.base.clone()),
            "root" => Some(ctx.steam_root.clone()),
            _ => None,
        },
        Flavor::Win => {
            if cfg!(windows) {
                win_native(token)
            } else if ctx.has_proton() {
                win_proton(token, &ctx.proton_drive_c)
            } else {
                None
            }
        }
        Flavor::Linux => {
            if cfg!(windows) {
                None
            } else {
                linux_native(token)
            }
        }
    }
}

fn env_or(key: &str, default: &str) -> PathBuf {
    PathBuf::from(std::env::var(key).unwrap_or_else(|_| default.to_string()))
}

fn win_native(token: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(match token {
        "home" => home,
        "winAppData" => dirs::data_dir()?,
        "winLocalAppData" => dirs::data_local_dir()?,
        "winLocalAppDataLow" => home.join("AppData").join("LocalLow"),
        "winDocuments" => dirs::document_dir().unwrap_or_else(|| home.join("Documents")),
        "winPublic" => env_or("PUBLIC", "C:\\Users\\Public"),
        "winProgramData" => env_or("PROGRAMDATA", "C:\\ProgramData"),
        "winDir" => env_or("WINDIR", "C:\\Windows"),
        _ => return None,
    })
}

fn win_proton(token: &str, dc: &Path) -> Option<PathBuf> {
    let home = dc.join("users").join("steamuser");
    Some(match token {
        "home" => home,
        "winAppData" => home.join("AppData").join("Roaming"),
        "winLocalAppData" => home.join("AppData").join("Local"),
        "winLocalAppDataLow" => home.join("AppData").join("LocalLow"),
        "winDocuments" => {
            let d = home.join("Documents");
            let md = home.join("My Documents");
            if !d.exists() && md.exists() { md } else { d }
        }
        "winPublic" => dc.join("users").join("Public"),
        "winProgramData" => dc.join("ProgramData"),
        "winDir" => dc.join("windows"),
        _ => return None,
    })
}

fn linux_native(token: &str) -> Option<PathBuf> {
    match token {
        "home" => dirs::home_dir(),
        "xdgData" => dirs::data_dir(),
        "xdgConfig" => dirs::config_dir(),
        _ => None,
    }
}

fn os_user_name(flavor: Flavor) -> String {
    if flavor == Flavor::Win && cfg!(not(windows)) {
        return "steamuser".into();
    }
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "*".into())
}

/// Tách `<token>/rest` → ("token", "/rest")
fn split_token(path: &str) -> Option<(&str, &str)> {
    let p = path.strip_prefix('<')?;
    let end = p.find('>')?;
    Some((&p[..end], &p[end + 1..]))
}

fn tags_ok(rule: &FileRule, include_config: bool) -> bool {
    rule.tags.is_empty()
        || rule.tags.iter().any(|t| t == "save")
        || (include_config && rule.tags.iter().any(|t| t == "config"))
}

fn rule_flavors(rule: &FileRule, token: &str) -> Vec<Flavor> {
    if token == "base" || token == "root" {
        return vec![Flavor::Any];
    }
    let (mut win, mut lin) = (false, false);
    if rule.when.is_empty() {
        win = true;
        lin = true;
    }
    for w in &rule.when {
        if let Some(s) = &w.store {
            if s != "steam" {
                continue;
            }
        }
        match w.os.as_deref() {
            Some("windows") => win = true,
            Some("linux") => lin = true,
            Some(_) => {}
            None => {
                win = true;
                lin = true;
            }
        }
    }
    if token.starts_with("win") {
        lin = false;
    }
    if token.starts_with("xdg") {
        win = false;
    }
    let mut v = Vec::new();
    if win {
        v.push(Flavor::Win);
    }
    if lin {
        v.push(Flavor::Linux);
    }
    v
}

#[derive(Debug, Clone)]
pub struct SaveFile {
    pub logical: String,
    pub real: PathBuf,
    pub size: u64,
    pub mtime_ms: i64,
}

#[derive(Debug, Default)]
pub struct LocalScan {
    pub files: Vec<SaveFile>,
    pub roots: Vec<String>,
}

impl LocalScan {
    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }
    pub fn latest_mtime(&self) -> Option<i64> {
        self.files.iter().map(|f| f.mtime_ms).max()
    }
}

fn rel_string(p: &Path, base: &Path) -> Option<String> {
    let rel = p.strip_prefix(base).ok()?;
    let parts: Vec<String> = rel
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();
    Some(parts.join("/"))
}

pub fn scan(ctx: &GameCtx, rules: &[FileRule], include_config: bool) -> LocalScan {
    let mut all: Vec<FileRule> = rules.to_vec();
    // Steam Cloud vẫn ghi save vào userdata/<account>/<appid>/remote dù đang offline
    all.push(FileRule {
        path: format!("<root>/userdata/*/{}/remote", ctx.appid),
        tags: vec!["save".into()],
        when: vec![],
    });

    let opts = MatchOptions { case_sensitive: false, require_literal_separator: true, require_literal_leading_dot: false };
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut scan = LocalScan::default();

    for rule in &all {
        if !tags_ok(rule, include_config) {
            continue;
        }
        let Some((token, rest)) = split_token(&rule.path) else { continue };
        // Không bao giờ backup nguyên thư mục cài game hay thư mục Steam
        if (token == "base" || token == "root") && rest.trim_matches('/').is_empty() {
            continue;
        }
        for flavor in rule_flavors(rule, token) {
            let Some(dir) = resolve(flavor, token, ctx) else { continue };
            if !dir.exists() {
                continue;
            }
            let rest = rest
                .replace("<storeUserId>", "*")
                .replace("<storeGameId>", &ctx.appid.to_string())
                .replace("<game>", &glob::Pattern::escape(&ctx.installdir))
                .replace("<osUserName>", &os_user_name(flavor));
            if rest.contains('<') {
                continue;
            }
            let pattern = format!("{}{}", glob::Pattern::escape(&dir.to_string_lossy()), rest);
            let Ok(paths) = glob::glob_with(&pattern, opts) else { continue };
            let mut matched_root = false;
            for p in paths.flatten() {
                let files: Vec<PathBuf> = if p.is_dir() {
                    WalkDir::new(&p).into_iter().flatten().filter(|e| e.file_type().is_file()).map(|e| e.into_path()).collect()
                } else if p.is_file() {
                    vec![p]
                } else {
                    vec![]
                };
                for f in files {
                    if !seen.insert(f.clone()) {
                        continue;
                    }
                    let Some(rel) = rel_string(&f, &dir) else { continue };
                    let Ok(md) = f.metadata() else { continue };
                    let mtime_ms = md
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0);
                    scan.files.push(SaveFile {
                        logical: format!("{}/{}/{}", flavor.tag(), token, rel),
                        real: f,
                        size: md.len(),
                        mtime_ms,
                    });
                    matched_root = true;
                }
            }
            if matched_root {
                let mut shown = format!("{}{}", dir.to_string_lossy(), rest);
                if cfg!(windows) {
                    shown = shown.replace('/', "\\");
                }
                if !scan.roots.contains(&shown) {
                    scan.roots.push(shown);
                }
            }
        }
    }
    scan.files.sort_by(|a, b| a.logical.cmp(&b.logical));
    scan
}

/// Đường dẫn logic → đường dẫn thật trên máy hiện tại.
pub fn logical_to_real(logical: &str, ctx: &GameCtx) -> Result<PathBuf> {
    let mut it = logical.splitn(3, '/');
    let (Some(fl), Some(token), Some(rel)) = (it.next(), it.next(), it.next()) else {
        bail!("Đường dẫn trong bản backup không hợp lệ: {logical}");
    };
    let flavor = Flavor::from_tag(fl).ok_or_else(|| anyhow!("Loại đường dẫn lạ: {fl}"))?;
    let dir = match resolve(flavor, token, ctx) {
        Some(d) => d,
        None => {
            if flavor == Flavor::Win && cfg!(not(windows)) {
                bail!("Chưa có Proton prefix cho game này. Chạy game 1 lần trên máy này rồi restore lại.");
            }
            if flavor == Flavor::Linux && cfg!(windows) {
                bail!("SKIP_LINUX_NATIVE");
            }
            bail!("Không xác định được thư mục <{token}> trên máy này");
        }
    };
    let mut out = dir;
    for part in rel.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains(':') {
            bail!("Đường dẫn trong bản backup không an toàn: {logical}");
        }
        out.push(part);
    }
    Ok(out)
}
