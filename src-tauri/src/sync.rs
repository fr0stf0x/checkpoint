//! Backup, restore, so sánh trạng thái local và cloud.
//!
//! Trên cloud:
//!   index.json                         danh sách phiên bản của mọi game (mới nhất đứng đầu)
//!   games/<appid>/<stamp>_<device>.zip file save + checkpoint-meta.json

use crate::config::{cache_dir, data_dir, Config, State, SyncBase};
use crate::manifest::{self, FileRule, Index};
use crate::paths::{self, GameCtx, LocalScan};
use crate::steam::{self, InstalledGame};
use crate::storage::{self, Storage};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const INDEX: &str = "index.json";
const META_IN_ZIP: &str = "checkpoint-meta.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionMeta {
    pub id: String,
    pub appid: u32,
    pub game_name: String,
    pub device_id: String,
    pub device_name: String,
    pub os: String,
    pub created_utc: i64,
    pub content_hash: String,
    pub files: usize,
    pub bytes: u64,
    pub zip: String,
    #[serde(default)]
    pub steam_ids: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CloudIndex {
    #[serde(default)]
    pub games: BTreeMap<String, Vec<VersionMeta>>,
}

#[derive(Debug, Serialize)]
pub struct OpResult {
    pub outcome: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct GameRow {
    pub appid: u32,
    pub name: String,
    pub installdir: String,
    pub library: String,
    pub has_rules: bool,
    pub roots: Vec<String>,
    pub files: usize,
    pub bytes: u64,
    pub local_hash: Option<String>,
    pub local_mtime: Option<i64>,
    pub cloud: Option<VersionMeta>,
    pub versions: usize,
    pub status: String,
    pub running: bool,
    pub steamid_bound: bool,
}

#[derive(Debug, Serialize)]
pub struct ListResult {
    pub games: Vec<GameRow>,
    pub cloud_error: Option<String>,
    pub steam_root: String,
    pub manifest_games: usize,
}

// ---------- manifest cache ----------

static MANIFEST: Mutex<Option<Arc<Index>>> = Mutex::new(None);

pub fn manifest() -> Option<Arc<Index>> {
    let mut g = MANIFEST.lock().unwrap();
    if g.is_none() {
        *g = manifest::load().map(Arc::new);
    }
    g.clone()
}

pub fn set_manifest(idx: Index) -> usize {
    let n = idx.games.len();
    *MANIFEST.lock().unwrap() = Some(Arc::new(idx));
    n
}

// ---------- môi trường Steam ----------

pub struct Env {
    pub root: PathBuf,
    pub games: Vec<InstalledGame>,
    pub manifest: Option<Arc<Index>>,
}

impl Env {
    pub fn load(cfg: &Config) -> Result<Env> {
        let root = steam::find_root(cfg.steam_root_override.as_deref())
            .context("Không tìm thấy thư mục Steam. Chỉ đường dẫn trong Cài đặt.")?;
        let libs = steam::libraries(&root);
        Ok(Env { games: steam::installed_games(&libs), root, manifest: manifest() })
    }

    fn game(&self, appid: u32) -> Result<&InstalledGame> {
        self.games.iter().find(|g| g.appid == appid).context("Game này không còn cài trên máy")
    }

    fn rules(&self, appid: u32) -> &[FileRule] {
        self.manifest.as_ref().and_then(|m| m.games.get(&appid)).map(|e| e.files.as_slice()).unwrap_or(&[])
    }

    fn ctx(&self, g: &InstalledGame) -> GameCtx {
        GameCtx::new(g.appid, &g.installdir, &g.library, &self.root)
    }
}

// ---------- tiện ích ----------

pub fn hash_scan(scan: &LocalScan) -> Result<Option<String>> {
    if scan.files.is_empty() {
        return Ok(None);
    }
    let mut h = Sha256::new();
    for f in &scan.files {
        h.update(f.logical.as_bytes());
        h.update([0u8]);
        let mut file = File::open(&f.real).with_context(|| format!("Không đọc được {}", f.real.display()))?;
        io::copy(&mut file, &mut h)?;
        h.update([0xffu8]);
    }
    Ok(Some(hex::encode(h.finalize())))
}

/// SteamID64 / AccountID xuất hiện trong đường dẫn save: dấu hiệu save gắn với account.
fn steam_ids(scan: &LocalScan) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for f in &scan.files {
        let parts: Vec<&str> = f.logical.split('/').collect();
        for (i, p) in parts.iter().enumerate() {
            let is64 = p.len() == 17 && p.starts_with("7656") && p.chars().all(|c| c.is_ascii_digit());
            let is_userdata = i > 0 && parts[i - 1] == "userdata" && p.chars().all(|c| c.is_ascii_digit());
            if (is64 || is_userdata) && !out.iter().any(|o| o == p) {
                out.push(p.to_string());
            }
        }
    }
    out
}

pub fn status(local: Option<&str>, cloud: Option<&VersionMeta>, base: Option<&SyncBase>, has_rules: bool) -> &'static str {
    match (local, cloud) {
        (None, None) => if has_rules { "empty" } else { "unknown" },
        (Some(_), None) => "new",
        (None, Some(_)) => "cloud",
        (Some(l), Some(c)) => {
            if l == c.content_hash {
                return "sync";
            }
            let Some(b) = base else { return "diverged" };
            match (l != b.local, c.content_hash != b.cloud) {
                (true, true) => "conflict",
                (true, false) => "local",
                (false, true) => "cloud",
                (false, false) => "sync",
            }
        }
    }
}

struct Procs(Vec<String>);

impl Procs {
    fn snapshot() -> Procs {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
        let mut sys = System::new();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::Always).with_cmd(UpdateKind::Always),
        );
        let mut v = Vec::new();
        for p in sys.processes().values() {
            let mut s = p.exe().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
            for a in p.cmd() {
                s.push(' ');
                s.push_str(&a.to_string_lossy());
            }
            v.push(norm(&s));
        }
        Procs(v)
    }
    fn running(&self, base: &Path) -> bool {
        let b = norm(&base.to_string_lossy());
        b.len() > 10 && self.0.iter().any(|s| s.contains(&b))
    }
}

fn norm(s: &str) -> String {
    let s = s.replace('\\', "/");
    if cfg!(windows) { s.to_lowercase() } else { s }
}

pub fn load_index(st: &dyn Storage) -> Result<CloudIndex> {
    match st.read(INDEX)? {
        Some(d) => serde_json::from_slice(&d).context("index.json trên cloud bị hỏng"),
        None => Ok(CloudIndex::default()),
    }
}

fn save_index(st: &dyn Storage, idx: &CloudIndex) -> Result<()> {
    st.write(INDEX, &serde_json::to_vec_pretty(idx)?)
}

fn os_name() -> &'static str {
    if cfg!(windows) { "windows" } else { "linux" }
}

fn write_zip(path: &Path, scan: &LocalScan, meta: Option<&VersionMeta>) -> Result<()> {
    use zip::write::SimpleFileOptions;
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    let mut zw = zip::ZipWriter::new(File::create(path)?);
    let opts = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(true);
    for f in &scan.files {
        zw.start_file(f.logical.clone(), opts)?;
        let mut src = File::open(&f.real).with_context(|| format!("Không đọc được {}", f.real.display()))?;
        io::copy(&mut src, &mut zw)?;
    }
    if let Some(m) = meta {
        zw.start_file(META_IN_ZIP, opts)?;
        io::Write::write_all(&mut zw, &serde_json::to_vec_pretty(m)?)?;
    }
    zw.finish()?;
    Ok(())
}

fn snapshot_local(appid: u32, scan: &LocalScan) -> Result<PathBuf> {
    let dir = data_dir().join("snapshots").join(appid.to_string());
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let path = dir.join(format!("{stamp}.zip"));
    write_zip(&path, scan, None)?;
    // giữ 5 snapshot gần nhất
    let mut all: Vec<PathBuf> = fs::read_dir(&dir)?.flatten().map(|e| e.path()).collect();
    all.sort();
    while all.len() > 5 {
        let _ = fs::remove_file(all.remove(0));
    }
    Ok(path)
}

fn fmt_time(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|t| t.with_timezone(&chrono::Local).format("%d/%m %H:%M").to_string())
        .unwrap_or_default()
}

// ---------- API ----------

pub fn list(cfg: &Config) -> Result<ListResult> {
    let env = Env::load(cfg)?;
    let state = State::load();
    let (idx, cloud_error) = match storage::open(&cfg.backend).and_then(|st| load_index(&*st)) {
        Ok(i) => (i, None),
        Err(e) => (CloudIndex::default(), Some(format!("{e:#}"))),
    };
    let procs = Procs::snapshot();
    let mut rows = Vec::new();
    for g in &env.games {
        let ctx = env.ctx(g);
        let rules = env.rules(g.appid);
        let scan = paths::scan(&ctx, rules, cfg.include_config);
        let local_hash = hash_scan(&scan).unwrap_or(None);
        let key = g.appid.to_string();
        let versions = idx.games.get(&key).map(|v| v.len()).unwrap_or(0);
        let cloud = idx.games.get(&key).and_then(|v| v.first()).cloned();
        let st = status(local_hash.as_deref(), cloud.as_ref(), state.games.get(&key), !rules.is_empty());
        let ids = steam_ids(&scan);
        rows.push(GameRow {
            appid: g.appid,
            name: g.name.clone(),
            installdir: g.installdir.clone(),
            library: g.library.to_string_lossy().to_string(),
            has_rules: !rules.is_empty(),
            roots: scan.roots.clone(),
            files: scan.files.len(),
            bytes: scan.bytes(),
            local_mtime: scan.latest_mtime(),
            local_hash,
            steamid_bound: !ids.is_empty() || cloud.as_ref().map(|c| !c.steam_ids.is_empty()).unwrap_or(false),
            cloud,
            versions,
            status: st.to_string(),
            running: procs.running(&ctx.base),
        });
    }
    Ok(ListResult {
        games: rows,
        cloud_error,
        steam_root: env.root.to_string_lossy().to_string(),
        manifest_games: env.manifest.as_ref().map(|m| m.games.len()).unwrap_or(0),
    })
}

pub fn backup(cfg: &Config, appid: u32, force: bool) -> Result<OpResult> {
    let env = Env::load(cfg)?;
    let g = env.game(appid)?;
    let ctx = env.ctx(g);
    if Procs::snapshot().running(&ctx.base) {
        bail!("{} đang chạy. Tắt game rồi backup.", g.name);
    }
    let scan = paths::scan(&ctx, env.rules(appid), cfg.include_config);
    let Some(hash) = hash_scan(&scan)? else {
        bail!("Không tìm thấy file save nào của {} trên máy này.", g.name);
    };
    let st = storage::open(&cfg.backend)?;
    let idx = load_index(&*st)?;
    let key = appid.to_string();
    let latest = idx.games.get(&key).and_then(|v| v.first()).cloned();
    let mut state = State::load();
    let now = chrono::Utc::now();

    if let Some(c) = &latest {
        if c.content_hash == hash {
            state.games.insert(key, SyncBase { local: hash.clone(), cloud: hash, at_utc: now.timestamp_millis() });
            state.save()?;
            return Ok(OpResult { outcome: "unchanged".into(), message: format!("{}: không có gì thay đổi, bỏ qua upload.", g.name) });
        }
    }
    let s = status(Some(&hash), latest.as_ref(), state.games.get(&key), true);
    if !force && matches!(s, "cloud" | "conflict" | "diverged") {
        bail!("NEEDS_CONFIRM:{s}");
    }

    let id = format!("{}_{}", now.format("%Y%m%dT%H%M%SZ"), cfg.device_id);
    let zip_rel = format!("games/{appid}/{id}.zip");
    let meta = VersionMeta {
        id: id.clone(),
        appid,
        game_name: g.name.clone(),
        device_id: cfg.device_id.clone(),
        device_name: cfg.device_name.clone(),
        os: os_name().into(),
        created_utc: now.timestamp_millis(),
        content_hash: hash.clone(),
        files: scan.files.len(),
        bytes: scan.bytes(),
        zip: zip_rel.clone(),
        steam_ids: steam_ids(&scan),
    };
    let tmp = cache_dir().join("tmp").join(format!("{id}.zip"));
    write_zip(&tmp, &scan, Some(&meta))?;
    let up = st.upload(&tmp, &zip_rel);
    let _ = fs::remove_file(&tmp);
    up?;

    // Đọc lại index ngay trước khi ghi để hạn chế đè lên thay đổi của máy khác
    let mut idx = load_index(&*st)?;
    let list = idx.games.entry(key.clone()).or_default();
    list.insert(0, meta);
    let keep = cfg.keep_versions.max(1);
    let pruned = if list.len() > keep { list.split_off(keep) } else { vec![] };
    save_index(&*st, &idx)?;
    for p in pruned {
        let _ = st.delete(&p.zip);
    }
    state.games.insert(key, SyncBase { local: hash.clone(), cloud: hash, at_utc: now.timestamp_millis() });
    state.save()?;
    Ok(OpResult {
        outcome: "uploaded".into(),
        message: format!("{}: đã backup {} file ({}).", g.name, scan.files.len(), human(scan.bytes())),
    })
}

pub fn restore(cfg: &Config, appid: u32, version_id: Option<String>, force: bool) -> Result<OpResult> {
    let env = Env::load(cfg)?;
    let g = env.game(appid)?;
    let ctx = env.ctx(g);
    if Procs::snapshot().running(&ctx.base) {
        bail!("{} đang chạy. Tắt game rồi restore.", g.name);
    }
    let rules = env.rules(appid);
    let scan = paths::scan(&ctx, rules, cfg.include_config);
    let local_hash = hash_scan(&scan)?;

    let st = storage::open(&cfg.backend)?;
    let idx = load_index(&*st)?;
    let key = appid.to_string();
    let versions = idx.games.get(&key).filter(|v| !v.is_empty()).context("Chưa có bản backup nào của game này trên cloud.")?;
    let latest = versions[0].clone();
    let ver = match &version_id {
        Some(id) => versions.iter().find(|v| &v.id == id).cloned().context("Không tìm thấy phiên bản đã chọn")?,
        None => latest.clone(),
    };
    let mut state = State::load();
    let now = chrono::Utc::now().timestamp_millis();

    if local_hash.as_deref() == Some(ver.content_hash.as_str()) {
        state.games.insert(key, SyncBase { local: ver.content_hash.clone(), cloud: latest.content_hash, at_utc: now });
        state.save()?;
        return Ok(OpResult { outcome: "unchanged".into(), message: format!("{}: bản local đã giống bản này, không cần restore.", g.name) });
    }
    let s = status(local_hash.as_deref(), Some(&latest), state.games.get(&key), true);
    if !force && local_hash.is_some() && matches!(s, "local" | "conflict" | "diverged") {
        bail!("NEEDS_CONFIRM:{s}");
    }

    if !scan.files.is_empty() {
        snapshot_local(appid, &scan).context("Không tạo được snapshot local trước khi restore")?;
    }

    let tmp = cache_dir().join("tmp").join(format!("restore-{}.zip", ver.id));
    if let Some(p) = tmp.parent() {
        fs::create_dir_all(p)?;
    }
    st.download(&ver.zip, &tmp)?;
    let result = extract(&tmp, &ctx);
    let _ = fs::remove_file(&tmp);
    let (restored, skipped) = result?;

    let after = hash_scan(&paths::scan(&ctx, rules, cfg.include_config))?.unwrap_or_default();
    state.games.insert(key, SyncBase { local: after, cloud: latest.content_hash, at_utc: now });
    state.save()?;
    let mut message = format!("{}: đã restore {} file từ bản của {} lúc {}.", g.name, restored, ver.device_name, fmt_time(ver.created_utc));
    if skipped > 0 {
        message.push_str(&format!(" Bỏ qua {skipped} file của bản Linux native (không dùng được trên Windows)."));
    }
    Ok(OpResult { outcome: "restored".into(), message })
}

fn extract(zip_path: &Path, ctx: &GameCtx) -> Result<(usize, usize)> {
    let mut za = zip::ZipArchive::new(File::open(zip_path)?).context("File backup bị hỏng")?;
    // Kiểm tra toàn bộ đường dẫn trước khi ghi bất kỳ file nào
    let mut plan = Vec::new();
    let mut skipped = 0;
    for i in 0..za.len() {
        let e = za.by_index(i)?;
        let name = e.name().to_string();
        if e.is_dir() || name == META_IN_ZIP {
            continue;
        }
        match paths::logical_to_real(&name, ctx) {
            Ok(p) => plan.push((i, p)),
            Err(err) if err.to_string() == "SKIP_LINUX_NATIVE" => skipped += 1,
            Err(err) => return Err(err),
        }
    }
    for (i, dest) in &plan {
        let mut e = za.by_index(*i)?;
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(dest).with_context(|| format!("Không ghi được {}", dest.display()))?;
        io::copy(&mut e, &mut out)?;
    }
    Ok((plan.len(), skipped))
}

pub fn versions(cfg: &Config, appid: u32) -> Result<Vec<VersionMeta>> {
    let st = storage::open(&cfg.backend)?;
    Ok(load_index(&*st)?.games.remove(&appid.to_string()).unwrap_or_default())
}

pub fn history(cfg: &Config) -> Result<Vec<VersionMeta>> {
    let st = storage::open(&cfg.backend)?;
    let mut all: Vec<VersionMeta> = load_index(&*st)?.games.into_values().flatten().collect();
    all.sort_by(|a, b| b.created_utc.cmp(&a.created_utc));
    all.truncate(100);
    Ok(all)
}

fn human(b: u64) -> String {
    let b = b as f64;
    if b >= 1e9 { format!("{:.2} GB", b / 1e9) }
    else if b >= 1e6 { format!("{:.1} MB", b / 1e6) }
    else if b >= 1e3 { format!("{:.0} KB", b / 1e3) }
    else { format!("{b} B") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(h: &str) -> VersionMeta {
        VersionMeta {
            id: "x".into(), appid: 1, game_name: "g".into(), device_id: "d".into(), device_name: "d".into(),
            os: "windows".into(), created_utc: 0, content_hash: h.into(), files: 1, bytes: 1, zip: "z".into(), steam_ids: vec![],
        }
    }
    fn base(l: &str, c: &str) -> SyncBase {
        SyncBase { local: l.into(), cloud: c.into(), at_utc: 0 }
    }

    /// Chạy trên máy thật: `cargo test smoke -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn smoke_real_machine() {
        if manifest().is_none() {
            set_manifest(manifest::download_and_build().expect("tải manifest"));
        }
        let state_path = crate::config::config_dir().join("state.json");
        let had_state = state_path.exists();
        let tmp = std::env::temp_dir().join("checkpoint-smoke");
        let _ = fs::remove_dir_all(&tmp);
        let mut cfg = Config::load();
        cfg.backend = crate::config::Backend::Folder { path: tmp.to_string_lossy().to_string() };

        let l = list(&cfg).expect("list");
        println!("steam root: {} · manifest {} game", l.steam_root, l.manifest_games);
        for g in &l.games {
            println!("{:>8} {:<40} {:<9} {:>4} file {:>10} B  {}", g.appid, g.name, g.status, g.files, g.bytes, g.roots.join(" | "));
        }
        let Some(g) = l.games.iter().filter(|g| g.local_hash.is_some() && !g.running).min_by_key(|g| g.bytes) else {
            println!("Không có game nào có save để thử backup");
            return;
        };
        let r = backup(&cfg, g.appid, false).expect("backup");
        println!("backup: {}", r.message);
        let again = backup(&cfg, g.appid, false).expect("backup lần 2");
        assert_eq!(again.outcome, "unchanged");
        let l2 = list(&cfg).expect("list 2");
        let row = l2.games.iter().find(|x| x.appid == g.appid).unwrap();
        assert_eq!(row.status, "sync");
        println!("sau backup: {} = {}", row.name, row.status);
        if !had_state {
            let _ = fs::remove_file(&state_path);
        } else {
            let mut st = State::load();
            st.games.remove(&g.appid.to_string());
            st.save().unwrap();
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    /// Backup thật lên nơi lưu đang cấu hình: `cargo test drive_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn drive_real() {
        let cfg = Config::load();
        let l = list(&cfg).expect("list");
        assert!(l.cloud_error.is_none(), "cloud lỗi: {:?}", l.cloud_error);
        let g = l.games.iter().filter(|g| g.local_hash.is_some() && !g.running).min_by_key(|g| g.bytes).expect("game có save");
        println!("game: {} ({} file, {} B), trạng thái trước: {}", g.name, g.files, g.bytes, g.status);
        let r = backup(&cfg, g.appid, false).expect("backup");
        println!("backup: {} -> {}", r.outcome, r.message);
        let l2 = list(&cfg).expect("list 2");
        let row = l2.games.iter().find(|x| x.appid == g.appid).unwrap();
        println!("sau backup: {}, {} bản trên cloud", row.status, row.versions);
        assert_eq!(row.status, "sync");

        let v = versions(&cfg, g.appid).expect("versions");
        let st = storage::open(&cfg.backend).unwrap();
        let tmp = std::env::temp_dir().join("checkpoint-drive-check.zip");
        st.download(&v[0].zip, &tmp).expect("download");
        let za = zip::ZipArchive::new(File::open(&tmp).unwrap()).unwrap();
        let names: Vec<String> = za.file_names().map(String::from).collect();
        println!("zip tải về: {} entry: {:?}", names.len(), names);
        assert!(names.iter().any(|n| n == META_IN_ZIP));
        assert_eq!(names.len(), v[0].files + 1);
        let _ = fs::remove_file(&tmp);

        let rr = restore(&cfg, g.appid, None, false).expect("restore");
        println!("restore cùng bản: {} -> {}", rr.outcome, rr.message);
        assert_eq!(rr.outcome, "unchanged");
    }

    #[test]
    fn zip_roundtrip_restores_files() {
        let root = std::env::temp_dir().join(format!("checkpoint-rt-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let lib = root.join("lib");
        let saves = lib.join("steamapps/common/MyGame/saves/slot1");
        fs::create_dir_all(&saves).unwrap();
        fs::write(saves.join("a.sav"), b"progress-1").unwrap();
        let ctx = GameCtx::new(42, "MyGame", &lib, &root.join("steam"));
        let rules = vec![FileRule { path: "<base>/saves".into(), tags: vec!["save".into()], when: vec![] }];
        let scan = paths::scan(&ctx, &rules, false);
        assert_eq!(scan.files.len(), 1);
        assert_eq!(scan.files[0].logical, "any/base/saves/slot1/a.sav");
        let h1 = hash_scan(&scan).unwrap().unwrap();
        let zip = root.join("b.zip");
        write_zip(&zip, &scan, None).unwrap();

        fs::write(saves.join("a.sav"), b"progress-2-newer").unwrap();
        let h2 = hash_scan(&paths::scan(&ctx, &rules, false)).unwrap().unwrap();
        assert_ne!(h1, h2);

        let (n, skipped) = extract(&zip, &ctx).unwrap();
        assert_eq!((n, skipped), (1, 0));
        assert_eq!(fs::read(saves.join("a.sav")).unwrap(), b"progress-1");
        assert_eq!(hash_scan(&paths::scan(&ctx, &rules, false)).unwrap().unwrap(), h1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_unsafe_paths() {
        let ctx = GameCtx::new(1, "G", Path::new("C:/lib"), Path::new("C:/steam"));
        assert!(paths::logical_to_real("any/base/../../evil.dll", &ctx).is_err());
        assert!(paths::logical_to_real("any/base/C:/x", &ctx).is_err());
        assert!(paths::logical_to_real("any/base/ok/file.sav", &ctx).is_ok());
    }

    #[test]
    fn status_cases() {
        assert_eq!(status(Some("a"), Some(&meta("a")), None, true), "sync");
        assert_eq!(status(Some("b"), Some(&meta("a")), Some(&base("a", "a")), true), "local");
        assert_eq!(status(Some("a"), Some(&meta("b")), Some(&base("a", "a")), true), "cloud");
        assert_eq!(status(Some("c"), Some(&meta("b")), Some(&base("a", "a")), true), "conflict");
        assert_eq!(status(Some("c"), Some(&meta("b")), None, true), "diverged");
        assert_eq!(status(Some("x"), Some(&meta("b")), Some(&base("x", "b")), true), "sync");
        assert_eq!(status(Some("a"), None, None, true), "new");
        assert_eq!(status(None, Some(&meta("a")), None, true), "cloud");
        assert_eq!(status(None, None, None, false), "unknown");
    }
}
