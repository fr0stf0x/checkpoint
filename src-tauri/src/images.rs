//! Account avatars and game artwork from Steam's local caches, served to the UI through the
//! `steamimg` URI scheme. Works offline: nothing is downloaded.
//!
//! URLs (built in the UI with `convertFileSrc(path, "steamimg")`):
//! `avatar/<SteamID64>`, `header/<appid>` (wide banner) and `capsule/<appid>` (portrait cover).
//! Only digits are accepted as IDs and file names come from fixed lists, so a request can never
//! read anything outside `config/avatarcache` and `appcache/librarycache`.

use crate::config::Config;
use crate::steam;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::http::{header, Response, StatusCode};

#[derive(Debug, PartialEq)]
pub enum Image {
    Avatar(u64),
    Header(u32),
    Capsule(u32),
}

/// Parse the request path, e.g. "/avatar%2F76561199698261377" or "/header/1245620".
pub fn parse(path: &str) -> Option<Image> {
    let p = path.trim_start_matches('/').replace("%2F", "/").replace("%2f", "/");
    let (kind, id) = p.split_once('/')?;
    if id.is_empty() || id.len() > 20 || !id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    match kind {
        "avatar" => id.parse().ok().map(Image::Avatar),
        "header" => id.parse().ok().map(Image::Header),
        "capsule" => id.parse().ok().map(Image::Capsule),
        _ => None,
    }
}

/// `name` directly in `dir`, or in one of its direct subfolders (newer Steam clients keep some
/// artwork in hash-named subfolders, e.g. `librarycache/<appid>/<hash>/library_capsule.jpg`).
fn find_in(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    for n in names {
        let p = dir.join(n);
        if p.is_file() {
            return Some(p);
        }
    }
    let mut subs: Vec<PathBuf> = fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    subs.sort();
    for n in names {
        if let Some(p) = subs.iter().map(|s| s.join(n)).find(|p| p.is_file()) {
            return Some(p);
        }
    }
    None
}

/// The file for an image under the Steam root, if Steam has it cached.
pub fn resolve(root: &Path, img: &Image) -> Option<PathBuf> {
    let lib = root.join("appcache").join("librarycache");
    let (appid, names, flat): (u32, &[&str], &[&str]) = match img {
        Image::Avatar(sid) => {
            let p = root.join("config").join("avatarcache").join(format!("{sid}.png"));
            return p.is_file().then_some(p);
        }
        // The hero image is a wide fallback for games that have no header (cropped by the UI).
        Image::Header(a) => (*a, &["header.jpg", "library_header.jpg", "library_hero.jpg"], &["header.jpg", "library_hero.jpg"]),
        Image::Capsule(a) => (*a, &["library_600x900.jpg", "library_capsule.jpg"], &["library_600x900.jpg"]),
    };
    let dir = lib.join(appid.to_string());
    if dir.is_dir() {
        if let Some(p) = find_in(&dir, names) {
            return Some(p);
        }
    }
    // Older Steam clients: flat files named "<appid>_header.jpg"
    flat.iter().map(|n| lib.join(format!("{appid}_{n}"))).find(|p| p.is_file())
}

/// Steam root for the current settings, remembered per override value (found once, not per image).
fn steam_root() -> Option<PathBuf> {
    static CACHE: Mutex<Option<(Option<String>, Option<PathBuf>)>> = Mutex::new(None);
    let ov = Config::load().steam_root_override;
    let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((k, v)) = c.as_ref() {
        if *k == ov {
            return v.clone();
        }
    }
    let root = steam::find_root(ov.as_deref());
    *c = Some((ov, root.clone()));
    root
}

fn mime(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    }
}

/// Answer one `steamimg` request: the image bytes, or 404 so the UI shows its placeholder.
pub fn respond(path: &str) -> Response<Vec<u8>> {
    let file = parse(path).and_then(|img| resolve(&steam_root()?, &img));
    let found = file.and_then(|p| fs::read(&p).ok().map(|b| (mime(&p), b)));
    let r = match found {
        Some((m, bytes)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, m)
            .header(header::CACHE_CONTROL, "max-age=3600")
            .body(bytes),
        None => Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()),
    };
    r.unwrap_or_else(|_| Response::new(Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_known_kinds_and_digit_ids() {
        assert_eq!(parse("/avatar%2F76561199698261377"), Some(Image::Avatar(76561199698261377)));
        assert_eq!(parse("/header/1245620"), Some(Image::Header(1245620)));
        assert_eq!(parse("capsule%2f4115450"), Some(Image::Capsule(4115450)));
        assert_eq!(parse("/header/..%2F..%2Fx"), None);
        assert_eq!(parse("/header/12a"), None);
        assert_eq!(parse("/icon/12"), None);
        assert_eq!(parse("/header/"), None);
        assert_eq!(parse("/header/99999999999"), None); // too big for u32
    }

    /// On a real machine: how many installed games and accounts have cached images.
    #[test]
    #[ignore]
    fn images_real() {
        let root = steam::find_root(None).expect("Steam not found");
        let games = steam::installed_games(&steam::libraries(&root));
        let (mut h, mut c) = (0, 0);
        for g in &games {
            let header = resolve(&root, &Image::Header(g.appid));
            let capsule = resolve(&root, &Image::Capsule(g.appid));
            h += header.is_some() as usize;
            c += capsule.is_some() as usize;
            if header.is_none() || capsule.is_none() {
                println!("{} {}: header={} capsule={}", g.appid, g.name, header.is_some(), capsule.is_some());
            }
        }
        let accts = steam::accounts(&root);
        let a = accts.iter().filter(|x| x.steamid64.parse().ok().and_then(|s| resolve(&root, &Image::Avatar(s))).is_some()).count();
        println!("games {}: header {h}, capsule {c}; accounts {}: avatar {a}", games.len(), accts.len());
    }

    #[test]
    fn resolves_all_cache_layouts() {
        let t = std::env::temp_dir().join(format!("cp-img-{}", std::process::id()));
        let lib = t.join("appcache").join("librarycache");
        fs::create_dir_all(lib.join("10").join("abc")).unwrap();
        fs::create_dir_all(lib.join("20")).unwrap();
        fs::create_dir_all(t.join("config").join("avatarcache")).unwrap();
        fs::write(lib.join("10").join("abc").join("library_capsule.jpg"), b"x").unwrap();
        fs::write(lib.join("10").join("abc").join("library_hero.jpg"), b"x").unwrap();
        fs::write(lib.join("20").join("header.jpg"), b"x").unwrap();
        fs::write(lib.join("30_header.jpg"), b"x").unwrap();
        fs::write(t.join("config").join("avatarcache").join("5.png"), b"x").unwrap();

        assert!(resolve(&t, &Image::Capsule(10)).unwrap().ends_with("abc/library_capsule.jpg"));
        assert!(resolve(&t, &Image::Header(10)).unwrap().ends_with("abc/library_hero.jpg"));
        assert!(resolve(&t, &Image::Header(20)).unwrap().ends_with("20/header.jpg"));
        assert!(resolve(&t, &Image::Header(30)).unwrap().ends_with("30_header.jpg"));
        assert_eq!(resolve(&t, &Image::Capsule(20)), None);
        assert!(resolve(&t, &Image::Avatar(5)).is_some());
        assert_eq!(resolve(&t, &Image::Avatar(6)), None);
        let _ = fs::remove_dir_all(&t);
    }
}
