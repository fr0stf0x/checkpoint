//! Moves a game's save from Steam account A to account B.
//!
//! Rules:
//! - COPY only. Nothing of A's is ever modified or deleted.
//! - B's files that get overwritten are captured first, and newly created folders are recorded,
//!   so "Undo" puts B back exactly as it was before the move.
//! - Each move is a record in `<data>/migrations/<id>.json` with state
//!   pending → kept | undone. The user tests in game, then keeps or undoes.

use crate::config::{data_dir, Config};
use crate::paths::{self, GameCtx, LocalScan, SaveFile};
use crate::storage;
use crate::sync::{self, Env, Procs};
use anyhow::{bail, Context, Result};
use crate::tr;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const STEAMID64_BASE: u64 = 76561197960265728;

/// An account in both forms: SteamID64 (game save folders) and AccountID (userdata, steam_autocloud.vdf).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ids {
    pub sid64: String,
    pub acct: String,
}

impl Ids {
    pub fn parse(s: &str) -> Result<Ids> {
        let n: u64 = s.trim().parse().with_context(|| tr!("err.bad_account_id", id = s).to_string())?;
        if n == 0 {
            bail!("{}", tr!("err.bad_account_id", id = s));
        }
        let (sid, acct) = if n >= STEAMID64_BASE { (n, n - STEAMID64_BASE) } else { (n + STEAMID64_BASE, n) };
        Ok(Ids { sid64: sid.to_string(), acct: acct.to_string() })
    }
}

/// Rewrite the path segments carrying `from`'s ID to `to`. None if the path is not tied to `from`.
pub fn remap_logical(logical: &str, from: &Ids, to: &Ids) -> Option<String> {
    let mut changed = false;
    let parts: Vec<String> = logical
        .split('/')
        .enumerate()
        .map(|(i, p)| {
            if i < 2 {
                return p.to_string(); // flavor/token
            }
            if p == from.acct {
                changed = true;
                return to.acct.clone();
            }
            if p.contains(&from.sid64) {
                changed = true;
                return p.replace(&from.sid64, &to.sid64);
            }
            p.to_string()
        })
        .collect();
    changed.then(|| parts.join("/"))
}

pub fn is_autocloud_vdf(p: &Path) -> bool {
    p.file_name().map(|n| n.to_string_lossy().eq_ignore_ascii_case("steam_autocloud.vdf")).unwrap_or(false)
}

/// steam_autocloud.vdf only holds `"accountid" "<AccountID>"`; Steam uses it to know who owns the folder.
pub fn rewrite_autocloud(data: &[u8], from: &Ids, to: &Ids) -> Vec<u8> {
    String::from_utf8_lossy(data)
        .replace(&format!("\"{}\"", from.acct), &format!("\"{}\"", to.acct))
        .replace(&from.sid64, &to.sid64)
        .into_bytes()
}

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
}

/// Forms a SteamID64 can take inside a file: ASCII text, UTF-16 text, little-endian u64.
/// Each pair has the same length, so replacing never shifts data.
fn id_patterns(from: &Ids, to: &Ids) -> Vec<(Vec<u8>, Vec<u8>)> {
    let f: u64 = from.sid64.parse().unwrap_or(0);
    let t: u64 = to.sid64.parse().unwrap_or(0);
    vec![
        (from.sid64.as_bytes().to_vec(), to.sid64.as_bytes().to_vec()),
        (utf16(&from.sid64), utf16(&to.sid64)),
        (f.to_le_bytes().to_vec(), t.to_le_bytes().to_vec()),
    ]
    .into_iter()
    .filter(|(a, b)| a.len() == b.len() && !a.is_empty())
    .collect()
}

fn count(hay: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || hay.len() < needle.len() {
        return 0;
    }
    hay.windows(needle.len()).filter(|w| *w == needle).count()
}

fn replace_same_len(data: &mut [u8], from: &[u8], to: &[u8]) -> usize {
    let mut n = 0;
    let mut i = 0;
    while i + from.len() <= data.len() {
        if &data[i..i + from.len()] == from {
            data[i..i + from.len()].copy_from_slice(to);
            n += 1;
            i += from.len();
        } else {
            i += 1;
        }
    }
    n
}

/// Shannon entropy in bits per byte. Close to 8 means compressed or encrypted.
fn entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut c = [0usize; 256];
    for &b in data {
        c[b as usize] += 1;
    }
    let n = data.len() as f64;
    c.iter().filter(|&&x| x > 0).map(|&x| {
        let p = x as f64 / n;
        -p * p.log2()
    }).sum()
}

// ---------- planning ----------

#[derive(Debug, Clone)]
struct Item {
    src: SaveFile,
    dst_logical: String,
    dst: PathBuf,
    exists: bool,
    hits: usize,
    autocloud: bool,
    entropy: f64,
}

#[derive(Debug, Serialize)]
pub struct FilePlan {
    pub from: String,
    pub to: String,
    pub size: u64,
    pub exists: bool,
    pub id_hits: usize,
    pub autocloud: bool,
}

#[derive(Debug, Serialize)]
pub struct Preview {
    pub appid: u32,
    pub name: String,
    pub from: String,
    pub to: String,
    /// none | folder | in_file | opaque
    pub class: String,
    pub files: Vec<FilePlan>,
    pub overwrite: usize,
    /// B already has a save folder for this game (usually means B launched it at least once)
    pub target_dir_exists: bool,
    pub running: bool,
    pub max_entropy: f64,
    pub pending_id: Option<String>,
}

fn plan_items(ctx: &GameCtx, scan: &LocalScan, from: &Ids, to: &Ids) -> Result<Vec<Item>> {
    let pats = id_patterns(from, to);
    let mut out = Vec::new();
    for f in &scan.files {
        let Some(dst_logical) = remap_logical(&f.logical, from, to) else { continue };
        let dst = paths::logical_to_real(&dst_logical, ctx)?;
        let autocloud = is_autocloud_vdf(&f.real);
        let (hits, ent) = if autocloud {
            (0, 0.0)
        } else {
            let data = fs::read(&f.real).with_context(|| tr!("err.read_file", path = f.real.display()).to_string())?;
            let sample = &data[..data.len().min(1 << 20)];
            (pats.iter().map(|(a, _)| count(&data, a)).sum(), entropy(sample))
        };
        out.push(Item { exists: dst.exists(), src: f.clone(), dst_logical, dst, hits, autocloud, entropy: ent });
    }
    Ok(out)
}

fn classify(items: &[Item]) -> &'static str {
    let data: Vec<&Item> = items.iter().filter(|i| !i.autocloud).collect();
    if items.is_empty() {
        "none"
    } else if data.iter().any(|i| i.hits > 0) {
        "in_file"
    } else if data.iter().any(|i| i.entropy > 7.5 && i.src.size >= 256) {
        "opaque"
    } else {
        "folder"
    }
}

/// B's account folder for the game (the first path segment whose ID was changed).
fn target_account_dir(item: &Item, ctx: &GameCtx) -> Option<PathBuf> {
    let src: Vec<&str> = item.src.logical.split('/').collect();
    let dst: Vec<&str> = item.dst_logical.split('/').collect();
    let idx = src.iter().zip(dst.iter()).position(|(a, b)| a != b)?;
    paths::logical_to_real(&dst[..=idx].join("/"), ctx).ok()
}

fn mig_dir() -> PathBuf {
    data_dir().join("migrations")
}

// ---------- move records ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    pub id: String,
    pub appid: u32,
    pub game_name: String,
    pub from: String,
    pub to: String,
    pub created_utc: i64,
    pub files: usize,
    pub patched: usize,
    /// pending | kept | undone
    pub state: String,
    pub created_files: Vec<PathBuf>,
    pub created_dirs: Vec<PathBuf>,
    pub overwritten: Vec<PathBuf>,
}

impl Migration {
    fn path(dir: &Path, id: &str) -> PathBuf {
        dir.join(format!("{id}.json"))
    }
    fn save(&self, dir: &Path) -> Result<()> {
        fs::create_dir_all(dir)?;
        fs::write(Self::path(dir, &self.id), serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }
    fn load(dir: &Path, id: &str) -> Result<Migration> {
        if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            bail!("{}", tr!("err.migration_bad_id"));
        }
        let d = fs::read(Self::path(dir, id)).context(tr!("err.migration_not_found").to_string())?;
        Ok(serde_json::from_slice(&d)?)
    }
}

fn list_in(dir: &Path) -> Vec<Migration> {
    let mut v: Vec<Migration> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
        .filter_map(|e| fs::read(e.path()).ok())
        .filter_map(|d| serde_json::from_slice::<Migration>(&d).ok())
        .collect();
    v.sort_by(|a, b| b.created_utc.cmp(&a.created_utc));
    v
}

/// Highest folder on the way to `p` that does not exist yet (it will be created when `p` is written).
fn topmost_missing(p: &Path) -> Option<PathBuf> {
    let mut cur = p.parent()?;
    let mut top = None;
    while !cur.exists() {
        top = Some(cur.to_path_buf());
        cur = cur.parent()?;
    }
    top
}

fn apply_items(dir: &Path, appid: u32, name: &str, from: &Ids, to: &Ids, items: &[Item], patch_in_file: bool) -> Result<Migration> {
    let now = chrono::Utc::now();
    let id = format!("{appid}-{}", now.format("%Y%m%dT%H%M%S%3fZ"));
    fs::create_dir_all(dir)?;

    // Capture B's files that are about to be overwritten
    let overwritten: Vec<PathBuf> = items.iter().filter(|i| i.exists).map(|i| i.dst.clone()).collect();
    if !overwritten.is_empty() {
        use zip::write::SimpleFileOptions;
        let mut zw = zip::ZipWriter::new(File::create(dir.join(format!("{id}.zip")))?);
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(true);
        for (n, p) in overwritten.iter().enumerate() {
            zw.start_file(n.to_string(), opts)?;
            io::copy(&mut File::open(p)?, &mut zw)?;
        }
        zw.finish()?;
    }

    let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
    for i in items.iter().filter(|i| !i.exists) {
        if let Some(t) = topmost_missing(&i.dst) {
            dirs.insert(t);
        }
    }
    // Drop sub-folders whose parent is also new
    let created_dirs: Vec<PathBuf> = dirs.iter().filter(|d| !dirs.iter().any(|o| o != *d && d.starts_with(o))).cloned().collect();

    let mut mig = Migration {
        id,
        appid,
        game_name: name.to_string(),
        from: from.sid64.clone(),
        to: to.sid64.clone(),
        created_utc: now.timestamp_millis(),
        files: items.len(),
        patched: 0,
        state: "pending".into(),
        created_files: items.iter().filter(|i| !i.exists).map(|i| i.dst.clone()).collect(),
        created_dirs,
        overwritten,
    };
    // Write the record before copying, so a failure halfway can still be undone
    mig.save(dir)?;

    let pats = id_patterns(from, to);
    for i in items {
        let mut data = fs::read(&i.src.real).with_context(|| tr!("err.read_file", path = i.src.real.display()).to_string())?;
        if i.autocloud {
            data = rewrite_autocloud(&data, from, to);
        } else if patch_in_file && i.hits > 0 {
            for (a, b) in &pats {
                replace_same_len(&mut data, a, b);
            }
            mig.patched += 1;
        }
        if let Some(parent) = i.dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&i.dst, &data).with_context(|| tr!("err.write_file", path = i.dst.display()).to_string())?;
    }
    mig.save(dir)?;
    Ok(mig)
}

fn undo_in(dir: &Path, id: &str) -> Result<Migration> {
    let mut m = Migration::load(dir, id)?;
    if m.state != "pending" {
        bail!("{}", tr!("err.migration_done"));
    }
    for d in &m.created_dirs {
        if d.exists() {
            fs::remove_dir_all(d).with_context(|| tr!("err.delete_path", path = d.display()).to_string())?;
        }
    }
    for f in &m.created_files {
        if f.exists() {
            fs::remove_file(f).with_context(|| tr!("err.delete_path", path = f.display()).to_string())?;
        }
    }
    if !m.overwritten.is_empty() {
        let zp = dir.join(format!("{}.zip", m.id));
        let mut za = zip::ZipArchive::new(File::open(&zp).context(tr!("err.migration_capture_missing").to_string())?)?;
        for (n, p) in m.overwritten.iter().enumerate() {
            let mut e = za.by_name(&n.to_string())?;
            let mut buf = Vec::new();
            e.read_to_end(&mut buf)?;
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(p, buf)?;
        }
    }
    m.state = "undone".into();
    m.save(dir)?;
    Ok(m)
}

// ---------- API for the UI ----------

struct Ready {
    name: String,
    ctx: GameCtx,
    items: Vec<Item>,
    running: bool,
}

fn prepare(cfg: &Config, appid: u32, from: &Ids, to: &Ids) -> Result<Ready> {
    if from == to {
        bail!("{}", tr!("err.same_account"));
    }
    let env = Env::load(cfg)?;
    let g = env.game(appid)?;
    let ctx = env.ctx(g);
    let scan = paths::scan(&ctx, env.rules(appid), cfg.include_config);
    let items = plan_items(&ctx, &scan, from, to)?;
    let running = Procs::snapshot().running(&ctx.base);
    Ok(Ready { name: g.name.clone(), ctx, items, running })
}

pub fn preview(cfg: &Config, appid: u32, from: &str, to: &str) -> Result<Preview> {
    let (f, t) = (Ids::parse(from)?, Ids::parse(to)?);
    let r = prepare(cfg, appid, &f, &t)?;
    let pending_id = list_in(&mig_dir()).into_iter().find(|m| m.appid == appid && m.state == "pending").map(|m| m.id);
    Ok(Preview {
        appid,
        name: r.name,
        from: f.sid64,
        to: t.sid64,
        class: classify(&r.items).into(),
        overwrite: r.items.iter().filter(|i| i.exists).count(),
        target_dir_exists: r.items.iter().filter_map(|i| target_account_dir(i, &r.ctx)).any(|d| d.exists()),
        running: r.running,
        max_entropy: r.items.iter().map(|i| i.entropy).fold(0.0, f64::max),
        files: r
            .items
            .iter()
            .map(|i| FilePlan {
                from: i.src.real.to_string_lossy().to_string(),
                to: i.dst.to_string_lossy().to_string(),
                size: i.src.size,
                exists: i.exists,
                id_hits: i.hits,
                autocloud: i.autocloud,
            })
            .collect(),
        pending_id,
    })
}

pub fn apply(cfg: &Config, appid: u32, from: &str, to: &str, patch_in_file: bool) -> Result<Migration> {
    let (f, t) = (Ids::parse(from)?, Ids::parse(to)?);
    if let Some(p) = list_in(&mig_dir()).into_iter().find(|m| m.appid == appid && m.state == "pending") {
        bail!("{}", tr!("err.migration_pending", game = p.game_name));
    }
    let r = prepare(cfg, appid, &f, &t)?;
    if r.running {
        bail!("{}", tr!("err.game_running", game = r.name));
    }
    if r.items.is_empty() {
        bail!("{}", tr!("err.migration_nothing", game = r.name));
    }
    apply_items(&mig_dir(), appid, &r.name, &f, &t, &r.items, patch_in_file)
}

pub fn undo(cfg: &Config, id: &str) -> Result<Migration> {
    let m = Migration::load(&mig_dir(), id)?;
    let env = Env::load(cfg)?;
    if let Ok(g) = env.game(m.appid) {
        if Procs::snapshot().running(&env.ctx(g).base) {
            bail!("{}", tr!("err.game_running", game = m.game_name));
        }
    }
    undo_in(&mig_dir(), id)
}

/// Keep the result. Stores the A → B pair in the cloud so other PCs restore A's backups into B.
/// Returns a warning (instead of failing) when the cloud write fails, since the local move is done.
pub fn keep(cfg: &Config, id: &str) -> Result<(Migration, Option<String>)> {
    let dir = mig_dir();
    let mut m = Migration::load(&dir, id)?;
    if m.state != "pending" {
        bail!("{}", tr!("err.migration_done"));
    }
    m.state = "kept".into();
    m.save(&dir)?;
    let _ = fs::remove_file(dir.join(format!("{}.zip", m.id)));
    let warn = storage::open(&cfg.backend)
        .and_then(|st| {
            let mut idx = sync::load_index(&*st)?;
            idx.account_aliases.insert(m.from.clone(), m.to.clone());
            sync::save_index(&*st, &idx)
        })
        .err()
        .map(|e| tr!("err.alias_not_saved", detail = format!("{e:#}")).to_string());
    Ok((m, warn))
}

pub fn pending() -> Vec<Migration> {
    list_in(&mig_dir()).into_iter().filter(|m| m.state == "pending").collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::FileRule;

    const A: &str = "76561198000000101";
    const B: &str = "76561198000000202";

    /// Read-only: `cargo test preview_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn preview_real() {
        let cfg = Config::load();
        let p = preview(&cfg, 4115450, "76561199698261377", "76561198396432414").expect("preview");
        println!("{} | class={} | files={} | overwrite={} | target_dir_exists={} | entropy={:.3} | running={}",
            p.name, p.class, p.files.len(), p.overwrite, p.target_dir_exists, p.max_entropy, p.running);
        for f in p.files.iter().take(4) {
            println!("  {} -> {} (hits {}, vdf {})", f.from, f.to, f.id_hits, f.autocloud);
        }
    }

    #[test]
    fn ids_both_forms() {
        let a = Ids::parse(A).unwrap();
        assert_eq!(a.acct, (76561198000000101u64 - STEAMID64_BASE).to_string());
        assert_eq!(Ids::parse(&a.acct).unwrap(), a);
    }

    #[test]
    fn remap_paths() {
        let (a, b) = (Ids::parse(A).unwrap(), Ids::parse(B).unwrap());
        assert_eq!(
            remap_logical(&format!("any/base/Saved/Steam/{A}/x.sav"), &a, &b).unwrap(),
            format!("any/base/Saved/Steam/{B}/x.sav")
        );
        assert_eq!(
            remap_logical(&format!("any/root/userdata/{}/1/remote/s", a.acct), &a, &b).unwrap(),
            format!("any/root/userdata/{}/1/remote/s", b.acct)
        );
        assert!(remap_logical("any/base/Saved/shared.sav", &a, &b).is_none());
    }

    fn setup(name: &str) -> (PathBuf, GameCtx, Vec<FileRule>, PathBuf) {
        let root = std::env::temp_dir().join(format!("checkpoint-mig-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let lib = root.join("lib");
        let a_dir = lib.join(format!("steamapps/common/G/Saved/Steam/{A}"));
        fs::create_dir_all(a_dir.join("1RealTime")).unwrap();
        // pseudo-random bytes standing in for an encrypted save
        let noise: Vec<u8> = (0..4096u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
        fs::write(a_dir.join("Save.sav"), &noise).unwrap();
        fs::write(a_dir.join("1RealTime/old.sav"), b"old").unwrap();
        let acct = Ids::parse(A).unwrap().acct;
        fs::write(a_dir.join("steam_autocloud.vdf"), format!("\"steam_autocloud.vdf\"\n{{\n\t\"accountid\"\t\t\"{acct}\"\n}}\n")).unwrap();
        fs::write(lib.join("steamapps/common/G/Saved/shared.cfg"), b"shared").unwrap();
        let ctx = GameCtx::new(7, "G", &lib, &root.join("steam"));
        let rules = vec![FileRule { path: "<base>/Saved".into(), tags: vec!["save".into()], when: vec![] }];
        (root, ctx, rules, a_dir)
    }

    #[test]
    fn apply_then_undo_restores_target() {
        let (root, ctx, rules, a_dir) = setup("undo");
        let (a, b) = (Ids::parse(A).unwrap(), Ids::parse(B).unwrap());
        let scan = paths::scan(&ctx, &rules, false);
        let items = plan_items(&ctx, &scan, &a, &b).unwrap();
        assert_eq!(items.len(), 3, "only files in A's folder, not shared.cfg");
        assert_eq!(classify(&items), "opaque");

        let dir = root.join("mig");
        let m = apply_items(&dir, 7, "G", &a, &b, &items, false).unwrap();
        let b_dir = a_dir.parent().unwrap().join(B);
        assert!(b_dir.join("Save.sav").exists());
        assert_eq!(fs::read(b_dir.join("Save.sav")).unwrap(), fs::read(a_dir.join("Save.sav")).unwrap());
        let vdf = fs::read_to_string(b_dir.join("steam_autocloud.vdf")).unwrap();
        assert!(vdf.contains(&format!("\"{}\"", b.acct)) && !vdf.contains(&a.acct));
        assert_eq!(m.created_dirs, vec![b_dir.clone()]);

        // the game creates more files while B tests it; undo must clean them up too
        fs::write(b_dir.join("new_by_game.sav"), b"x").unwrap();
        undo_in(&dir, &m.id).unwrap();
        assert!(!b_dir.exists());
        assert!(a_dir.join("Save.sav").exists(), "A's copy untouched");
        assert!(undo_in(&dir, &m.id).is_err(), "cannot undo twice");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn undo_restores_overwritten_files_and_patch_works() {
        let (root, ctx, rules, a_dir) = setup("over");
        let (a, b) = (Ids::parse(A).unwrap(), Ids::parse(B).unwrap());
        // ID stored as text inside the file
        fs::write(a_dir.join("Save.sav"), format!("player={A};lvl=9")).unwrap();
        let b_dir = a_dir.parent().unwrap().join(B);
        fs::create_dir_all(&b_dir).unwrap();
        fs::write(b_dir.join("Save.sav"), b"b-own-progress").unwrap();

        let scan = paths::scan(&ctx, &rules, false);
        let items = plan_items(&ctx, &scan, &a, &b).unwrap();
        assert_eq!(classify(&items), "in_file");
        let dir = root.join("mig");
        let m = apply_items(&dir, 7, "G", &a, &b, &items, true).unwrap();
        assert_eq!(m.patched, 1);
        assert_eq!(fs::read_to_string(b_dir.join("Save.sav")).unwrap(), format!("player={B};lvl=9"));
        assert_eq!(fs::read_to_string(a_dir.join("Save.sav")).unwrap(), format!("player={A};lvl=9"));

        undo_in(&dir, &m.id).unwrap();
        assert_eq!(fs::read(b_dir.join("Save.sav")).unwrap(), b"b-own-progress");
        assert!(!b_dir.join("1RealTime").exists());
        let _ = fs::remove_dir_all(&root);
    }
}
