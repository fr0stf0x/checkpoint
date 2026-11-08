//! "Add to Steam library": registers Checkpoint as a non-Steam game, so it can be started from
//! the Steam library and from Gaming Mode on Bazzite / SteamOS.
//!
//! Non-Steam games live in `userdata/<accountid>/config/shortcuts.vdf`, Valve's binary KeyValues:
//! `0x00 key\0 … 0x08` is a map, `0x01 key\0 value\0` a string, `0x02 key\0 <u32 LE>` an integer.
//! Steam keeps the file in memory and rewrites it when it exits, so Steam must be closed while
//! we write. Strings are kept as raw bytes and unknown value types are refused, so the user's
//! other shortcuts are written back byte for byte. A copy of the old file is kept next to it.

use crate::tr;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const APP_NAME: &str = "Checkpoint";

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Map(Vec<(Vec<u8>, Node)>),
    Str(Vec<u8>),
    Int(u32),
    /// Other fixed-size types (float, pointer, color, 64-bit), kept as they are.
    Raw(u8, Vec<u8>),
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Option<u8> {
        let v = *self.b.get(self.i)?;
        self.i += 1;
        Some(v)
    }
    fn take(&mut self, n: usize) -> Option<Vec<u8>> {
        let v = self.b.get(self.i..self.i + n)?.to_vec();
        self.i += n;
        Some(v)
    }
    fn cstr(&mut self) -> Option<Vec<u8>> {
        let end = self.b[self.i..].iter().position(|&c| c == 0)?;
        let v = self.b[self.i..self.i + end].to_vec();
        self.i += end + 1;
        Some(v)
    }
    fn map(&mut self) -> Option<Vec<(Vec<u8>, Node)>> {
        let mut out = Vec::new();
        loop {
            let t = self.byte()?;
            if t == 0x08 {
                return Some(out);
            }
            let key = self.cstr()?;
            let node = match t {
                0x00 => Node::Map(self.map()?),
                0x01 => Node::Str(self.cstr()?),
                0x02 => Node::Int(u32::from_le_bytes(self.take(4)?.try_into().ok()?)),
                0x03 | 0x04 | 0x06 => Node::Raw(t, self.take(4)?),
                0x07 | 0x0A => Node::Raw(t, self.take(8)?),
                _ => return None,
            };
            out.push((key, node));
        }
    }
}

fn parse(data: &[u8]) -> Option<Vec<(Vec<u8>, Node)>> {
    let mut r = Reader { b: data, i: 0 };
    let root = r.map()?;
    // Anything after the root's end marker means a format we do not understand
    (r.i == data.len()).then_some(root)
}

fn write_map(entries: &[(Vec<u8>, Node)], out: &mut Vec<u8>) {
    for (k, v) in entries {
        let t = match v {
            Node::Map(_) => 0x00,
            Node::Str(_) => 0x01,
            Node::Int(_) => 0x02,
            Node::Raw(t, _) => *t,
        };
        out.push(t);
        out.extend_from_slice(k);
        out.push(0);
        match v {
            Node::Map(m) => write_map(m, out),
            Node::Str(s) => {
                out.extend_from_slice(s);
                out.push(0);
            }
            Node::Int(n) => out.extend_from_slice(&n.to_le_bytes()),
            Node::Raw(_, b) => out.extend_from_slice(b),
        }
    }
    out.push(0x08);
}

fn serialize(root: &[(Vec<u8>, Node)]) -> Vec<u8> {
    let mut out = Vec::new();
    write_map(root, &mut out);
    out
}

fn get_str<'a>(m: &'a [(Vec<u8>, Node)], key: &str) -> Option<&'a [u8]> {
    m.iter().find(|(k, _)| k.eq_ignore_ascii_case(key.as_bytes())).and_then(|(_, v)| match v {
        Node::Str(s) => Some(s.as_slice()),
        _ => None,
    })
}

fn set(m: &mut Vec<(Vec<u8>, Node)>, key: &str, val: Node) {
    match m.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(key.as_bytes())) {
        Some((_, v)) => *v = val,
        None => m.push((key.as_bytes().to_vec(), val)),
    }
}

fn s(v: &str) -> Node {
    Node::Str(v.as_bytes().to_vec())
}

/// Steam's shortcut ID: any value with the top bit set works; derive it from the path so it is stable.
fn shortcut_appid(exe: &str) -> u32 {
    let h = Sha256::digest(format!("{exe}{APP_NAME}").as_bytes());
    u32::from_le_bytes([h[0], h[1], h[2], h[3]]) | 0x8000_0000
}

fn quoted(p: &str) -> String {
    format!("\"{p}\"")
}

/// Add Checkpoint to `root` (the parsed file), or update its entry when it is already there
/// (e.g. the AppImage was moved). Returns true when a new entry was added.
fn upsert(root: &mut Vec<(Vec<u8>, Node)>, exe: &str, start_dir: &str, icon: &str) -> Result<bool> {
    if !root.iter().any(|(k, _)| k.eq_ignore_ascii_case(b"shortcuts")) {
        root.push((b"shortcuts".to_vec(), Node::Map(Vec::new())));
    }
    let Some((_, Node::Map(list))) = root.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(b"shortcuts")) else {
        bail!("{}", tr!("err.shortcuts_format"));
    };
    let exe_q = quoted(exe);
    let existing = list.iter_mut().find_map(|(_, v)| match v {
        Node::Map(m) => {
            let name = get_str(m, "AppName").map(|n| n.eq_ignore_ascii_case(APP_NAME.as_bytes())).unwrap_or(false);
            let same_exe = get_str(m, "Exe").map(|e| e == exe_q.as_bytes()).unwrap_or(false);
            (name || same_exe).then_some(m)
        }
        _ => None,
    });
    if let Some(m) = existing {
        set(m, "Exe", s(&exe_q));
        set(m, "StartDir", s(&quoted(start_dir)));
        set(m, "icon", s(icon));
        return Ok(false);
    }
    // Entries are keyed "0", "1", …: use the next free number
    let next = (0..).find(|n: &usize| !list.iter().any(|(k, _)| k == n.to_string().as_bytes())).unwrap_or(list.len());
    let entry = vec![
        (b"appid".to_vec(), Node::Int(shortcut_appid(exe))),
        (b"AppName".to_vec(), s(APP_NAME)),
        (b"Exe".to_vec(), s(&exe_q)),
        (b"StartDir".to_vec(), s(&quoted(start_dir))),
        (b"icon".to_vec(), s(icon)),
        (b"ShortcutPath".to_vec(), s("")),
        (b"LaunchOptions".to_vec(), s("")),
        (b"IsHidden".to_vec(), Node::Int(0)),
        (b"AllowDesktopConfig".to_vec(), Node::Int(1)),
        (b"AllowOverlay".to_vec(), Node::Int(1)),
        (b"OpenVR".to_vec(), Node::Int(0)),
        (b"Devkit".to_vec(), Node::Int(0)),
        (b"DevkitGameID".to_vec(), s("")),
        (b"DevkitOverrideAppID".to_vec(), Node::Int(0)),
        (b"LastPlayTime".to_vec(), Node::Int(0)),
        (b"FlatpakAppID".to_vec(), s("")),
        (b"tags".to_vec(), Node::Map(Vec::new())),
    ];
    list.push((next.to_string().into_bytes(), Node::Map(entry)));
    Ok(true)
}

/// The file Steam should start: the AppImage itself on Linux (not the copy mounted in /tmp),
/// otherwise this executable.
pub fn launcher() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("APPIMAGE").filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    std::env::current_exe().context(tr!("err.exe_path").to_string())
}

/// Steam's own process (not the web helpers, which exit with it).
fn steam_running() -> bool {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    sys.processes().values().any(|p| {
        let n = p.name().to_string_lossy().to_lowercase();
        n == "steam.exe" || n == "steam"
    })
}

#[derive(Debug, Serialize)]
pub struct AddResult {
    /// false: Checkpoint was already in the library and its entry was updated
    pub added: bool,
    pub account: String,
}

pub fn add_to_steam(steam_root: &Path) -> Result<AddResult> {
    if steam_running() {
        bail!("{}", tr!("err.steam_running"));
    }
    let accts = crate::steam::accounts(steam_root);
    let acct = accts.iter().find(|a| a.most_recent).context(tr!("err.no_steam_account").to_string())?;
    let dir = steam_root.join("userdata").join(acct.account_id.to_string()).join("config");
    let path = dir.join("shortcuts.vdf");

    let mut root = match fs::read(&path) {
        Ok(data) if !data.is_empty() => parse(&data).context(tr!("err.shortcuts_format").to_string())?,
        _ => Vec::new(),
    };
    let exe = launcher()?;
    let start_dir = exe.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let icon = write_icon().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let added = upsert(&mut root, &exe.to_string_lossy(), &start_dir, &icon)?;

    fs::create_dir_all(&dir).with_context(|| tr!("err.create_folder", path = dir.display()).to_string())?;
    if path.exists() {
        fs::copy(&path, dir.join("shortcuts.vdf.checkpoint-bak"))
            .with_context(|| tr!("err.write_file", path = dir.join("shortcuts.vdf.checkpoint-bak").display()).to_string())?;
    }
    let tmp = dir.join("shortcuts.vdf.checkpoint-tmp");
    fs::write(&tmp, serialize(&root)).with_context(|| tr!("err.write_file", path = tmp.display()).to_string())?;
    fs::rename(&tmp, &path).with_context(|| tr!("err.write_file", path = path.display()).to_string())?;
    let name = if acct.persona_name.is_empty() { acct.account_name.clone() } else { acct.persona_name.clone() };
    Ok(AddResult { added, account: name })
}

/// The app icon as a PNG file Steam can show in the library.
fn write_icon() -> Option<PathBuf> {
    let p = crate::config::data_dir().join("checkpoint-icon.png");
    fs::create_dir_all(p.parent()?).ok()?;
    fs::write(&p, include_bytes!("../icons/128x128@2x.png")).ok()?;
    Some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file as Steam writes it: one existing shortcut with a non-UTF-8 byte and a tag.
    fn sample() -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"\x00shortcuts\x00");
        b.extend_from_slice(b"\x000\x00");
        b.extend_from_slice(b"\x02appid\x00\x01\x02\x03\x84");
        b.extend_from_slice(b"\x01AppName\x00Caf\xe9 Emu\x00");
        b.extend_from_slice(b"\x01Exe\x00\"C:\\Emu\\emu.exe\"\x00");
        b.extend_from_slice(b"\x02LastPlayTime\x00\x10\x20\x30\x40");
        b.extend_from_slice(b"\x00tags\x00\x010\x00favorite\x00\x08");
        b.extend_from_slice(b"\x08");
        b.extend_from_slice(b"\x08");
        b.extend_from_slice(b"\x08");
        b
    }

    #[test]
    fn round_trips_byte_for_byte() {
        let data = sample();
        let root = parse(&data).expect("parse");
        assert_eq!(serialize(&root), data);
    }

    #[test]
    fn adds_once_then_updates() {
        let mut root = parse(&sample()).unwrap();
        assert!(upsert(&mut root, "/home/u/Apps/Checkpoint.AppImage", "/home/u/Apps", "/x/icon.png").unwrap());
        // Moved AppImage: same entry, new path
        assert!(!upsert(&mut root, "/home/u/Checkpoint.AppImage", "/home/u", "/x/icon.png").unwrap());
        let again = parse(&serialize(&root)).unwrap();
        let Node::Map(list) = &again[0].1 else { panic!() };
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].0, b"1");
        let Node::Map(m) = &list[1].1 else { panic!() };
        assert_eq!(get_str(m, "Exe"), Some(&b"\"/home/u/Checkpoint.AppImage\""[..]));
        assert_eq!(get_str(m, "AppName"), Some(&b"Checkpoint"[..]));
        // The other shortcut is untouched
        let Node::Map(first) = &list[0].1 else { panic!() };
        assert_eq!(get_str(first, "AppName"), Some(&b"Caf\xe9 Emu"[..]));
    }

    #[test]
    fn new_file_and_bad_files() {
        let mut root = Vec::new();
        assert!(upsert(&mut root, "C:\\Apps\\checkpoint.exe", "C:\\Apps", "").unwrap());
        let data = serialize(&root);
        assert!(data.starts_with(b"\x00shortcuts\x00\x000\x00\x02appid\x00"));
        assert!(data.ends_with(b"\x08\x08\x08"));
        assert!(parse(&data[..data.len() - 1]).is_none(), "truncated file must be refused");
        assert!(parse(b"\x00shortcuts\x00\x09x\x00\x08\x08").is_none(), "unknown type must be refused");
        assert!(shortcut_appid("x") & 0x8000_0000 != 0);
    }
}
