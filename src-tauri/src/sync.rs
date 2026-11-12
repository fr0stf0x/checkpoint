//! Backup, restore and local-vs-cloud status.
//!
//! In the cloud:
//!   index.json                         every version of every game (newest first)
//!   games/<appid>/<stamp>_<device>.zip save files + checkpoint-meta.json

use crate::config::{cache_dir, data_dir, Config, State, SyncBase};
use crate::manifest::{self, FileRule, Index};
use crate::paths::{self, GameCtx, LocalScan};
use crate::steam::{self, InstalledGame};
use crate::storage::{self, Storage};
use anyhow::{bail, Context, Result};
use crate::tr;
use rust_i18n::t;
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
    /// Set when a backup replaced this version while it had progress this PC didn't have (a
    /// conflict, or "back up anyway" over a newer cloud version). Kept versions are not counted
    /// in `keep_versions` and never removed automatically.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept: Option<Kept>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Kept {
    /// The PC whose backup replaced this version
    pub by_device: String,
    pub at_utc: i64,
}

/// Drop the oldest versions beyond `keep` (the list is newest first). Kept versions don't count
/// and always stay. Returns the removed versions, whose zips the caller deletes.
fn prune(list: &mut Vec<VersionMeta>, keep: usize) -> Vec<VersionMeta> {
    let keep = keep.max(1);
    let (mut n, mut removed) = (0, Vec::new());
    list.retain(|v| {
        if v.kept.is_some() {
            return true;
        }
        n += 1;
        if n <= keep {
            return true;
        }
        removed.push(v.clone());
        false
    });
    removed
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CloudIndex {
    #[serde(default)]
    pub games: BTreeMap<String, Vec<VersionMeta>>,
    /// Old account → new account (SteamID64), stored when a save move A → B is kept.
    /// Restoring A's backup on another PC then defaults to writing into B.
    #[serde(default)]
    pub account_aliases: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct OpResult {
    pub outcome: String,
    pub message: String,
}

/// Progress of a backup or restore, shown in the UI as a list of save files.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Progress {
    /// A step starts: "snapshot", "pack", "upload", "download" or "extract".
    Step { step: &'static str },
    /// The save files being packed (backup) or written (restore), in order.
    Files { files: Vec<ProgressFile> },
    /// File `index` of the list is being packed or written; the ones before it are done.
    File { index: usize },
    /// Upload or download: bytes so far of `total` (0 while unknown).
    Bytes { done: u64, total: u64 },
    /// Every step of the operation up front (restore), so the UI lists them before they run.
    Steps { steps: Vec<StepInfo> },
    /// Files done in a step that has no file list (the snapshot before a restore).
    Count { done: usize, total: usize },
}

#[derive(Debug, Clone, Serialize)]
pub struct StepInfo {
    pub step: &'static str,
    pub files: usize,
    pub bytes: u64,
    /// Download: the PC that made the backup, and when
    pub device: Option<String>,
    pub time: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgressFile {
    pub name: String,
    /// Folder with a trailing separator
    pub dir: String,
    pub size: u64,
}

impl ProgressFile {
    fn new(path: &Path, size: u64) -> Self {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let mut dir = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        if !dir.is_empty() && !dir.ends_with(std::path::MAIN_SEPARATOR) {
            dir.push(std::path::MAIN_SEPARATOR);
        }
        Self { name, dir, size }
    }
}

pub type OnProgress<'a> = &'a (dyn Fn(Progress) + Sync);

/// For callers without a window (launch wrapper, tests).
pub fn no_progress(_: Progress) {}

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
    /// SteamID64 / AccountID found in this machine's save paths
    pub steam_ids: Vec<String>,
    /// Local snapshots kept for this game (taken before restores)
    pub snapshots: usize,
}

#[derive(Debug, Serialize)]
pub struct ListResult {
    pub games: Vec<GameRow>,
    pub cloud_error: Option<String>,
    pub steam_root: String,
    pub manifest_games: usize,
    pub account_aliases: BTreeMap<String, String>,
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

// ---------- Steam environment ----------

pub struct Env {
    pub root: PathBuf,
    pub games: Vec<InstalledGame>,
    pub manifest: Option<Arc<Index>>,
}

impl Env {
    pub fn load(cfg: &Config) -> Result<Env> {
        let root = steam::find_root(cfg.steam_root_override.as_deref())
            .context(tr!("err.no_steam").to_string())?;
        let libs = steam::libraries(&root);
        Ok(Env { games: steam::installed_games(&libs), root, manifest: manifest() })
    }

    pub(crate) fn game(&self, appid: u32) -> Result<&InstalledGame> {
        self.games.iter().find(|g| g.appid == appid).context(tr!("err.game_not_installed").to_string())
    }

    pub(crate) fn rules(&self, appid: u32) -> &[FileRule] {
        self.manifest.as_ref().and_then(|m| m.games.get(&appid)).map(|e| e.files.as_slice()).unwrap_or(&[])
    }

    pub(crate) fn ctx(&self, g: &InstalledGame) -> GameCtx {
        GameCtx::new(g.appid, &g.installdir, &g.library, &self.root)
    }
}

// ---------- helpers ----------

pub fn hash_scan(scan: &LocalScan) -> Result<Option<String>> {
    if scan.files.is_empty() {
        return Ok(None);
    }
    let mut h = Sha256::new();
    for f in &scan.files {
        h.update(f.logical.as_bytes());
        h.update([0u8]);
        let mut file = File::open(&f.real).with_context(|| tr!("err.read_file", path = f.real.display()).to_string())?;
        io::copy(&mut file, &mut h)?;
        h.update([0xffu8]);
    }
    Ok(Some(hex::encode(h.finalize())))
}

/// SteamID64 / AccountID present in save paths: a sign the save is tied to an account.
pub(crate) fn steam_ids(scan: &LocalScan) -> Vec<String> {
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

pub(crate) struct Procs(Vec<String>);

impl Procs {
    pub(crate) fn snapshot() -> Procs {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
        let mut sys = System::new();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::Always).with_cmd(UpdateKind::Always),
        );
        // Some processes name the game folder on their command line without being the game:
        // this process and the ones that started it (the launch wrapper itself, Steam's reaper
        // on Linux), and any other Checkpoint launch wrapper. Counting them would make the
        // wrapper wait for itself and show the game as running until the wrapper ends.
        let mut skip = std::collections::HashSet::new();
        let mut cur = Some(sysinfo::Pid::from_u32(std::process::id()));
        while let Some(pid) = cur {
            if !skip.insert(pid) {
                break;
            }
            cur = sys.process(pid).and_then(|p| p.parent());
        }
        let mut v = Vec::new();
        for (pid, p) in sys.processes() {
            // On Linux, sysinfo also lists every thread as a task: the process itself is enough
            if skip.contains(pid) || p.thread_kind().is_some() {
                continue;
            }
            let exe = p.exe().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
            let args: Vec<String> = p.cmd().iter().map(|a| a.to_string_lossy().to_string()).collect();
            if is_launch_wrapper(&exe, &args) {
                continue;
            }
            let mut s = exe;
            for a in &args {
                s.push(' ');
                s.push_str(a);
            }
            v.push(norm(&s));
        }
        Procs(v)
    }
    pub(crate) fn running(&self, base: &Path) -> bool {
        let b = norm(&base.to_string_lossy());
        b.len() > 10 && self.0.iter().any(|s| s.contains(&b))
    }
}

/// `checkpoint run …` (also from an AppImage, e.g. `Checkpoint_0.7.0_amd64.AppImage run …`).
fn is_launch_wrapper(exe: &str, args: &[String]) -> bool {
    let stem = |p: &str| Path::new(&p.replace('\\', "/")).file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let named = stem(exe).starts_with("checkpoint") || args.first().is_some_and(|a| stem(a).starts_with("checkpoint"));
    named && args.get(1).is_some_and(|a| a == "run")
}

fn norm(s: &str) -> String {
    let s = s.replace('\\', "/");
    if cfg!(windows) { s.to_lowercase() } else { s }
}

pub fn load_index(st: &dyn Storage) -> Result<CloudIndex> {
    match st.read(INDEX)? {
        Some(d) => serde_json::from_slice(&d).context(tr!("err.index_corrupt").to_string()),
        None => Ok(CloudIndex::default()),
    }
}

pub(crate) fn save_index(st: &dyn Storage, idx: &CloudIndex) -> Result<()> {
    st.write(INDEX, &serde_json::to_vec_pretty(idx)?)
}

fn os_name() -> &'static str {
    if cfg!(windows) { "windows" } else { "linux" }
}

/// `meta`: (file name, JSON) stored next to the save files.
fn write_zip(path: &Path, scan: &LocalScan, meta: Option<(&str, &[u8])>, on: OnProgress) -> Result<()> {
    use zip::write::SimpleFileOptions;
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    let mut zw = zip::ZipWriter::new(File::create(path)?);
    let opts = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(true);
    for (index, f) in scan.files.iter().enumerate() {
        on(Progress::File { index });
        zw.start_file(f.logical.clone(), opts)?;
        let mut src = File::open(&f.real).with_context(|| tr!("err.read_file", path = f.real.display()).to_string())?;
        io::copy(&mut src, &mut zw)?;
    }
    if let Some((name, data)) = meta {
        zw.start_file(name, opts)?;
        io::Write::write_all(&mut zw, data)?;
    }
    zw.finish()?;
    Ok(())
}

// ---------- local snapshots ----------
//
// Before anything overwrites a save on this PC (restoring a cloud version, or going back to a
// snapshot), the save is zipped to <data>/snapshots/<appid>/<stamp>.zip with the same logical
// paths as a backup, plus checkpoint-snapshot.json. Snapshots made by versions before 0.7 have no
// JSON; their details are read from the zip itself.

const SNAP_META: &str = "checkpoint-snapshot.json";
/// Snapshots kept per game; the oldest go first.
pub const KEEP_SNAPSHOTS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMeta {
    /// File name without `.zip`
    #[serde(default)]
    pub id: String,
    pub created_utc: i64,
    /// Same hash as `hash_scan`, so it can be compared with the save on this PC
    #[serde(default)]
    pub content_hash: Option<String>,
    pub files: usize,
    pub bytes: u64,
    /// Newest file date in the save: roughly when it was last played
    #[serde(default)]
    pub save_mtime: Option<i64>,
    /// What replaced this save: "restore" (a cloud version), "snapshot" (going back to another
    /// snapshot), "" (unknown: made by an older version)
    #[serde(default)]
    pub reason: String,
    /// The cloud version's PC, for "restore"
    #[serde(default)]
    pub source_device: Option<String>,
    /// The cloud version's or the other snapshot's time
    #[serde(default)]
    pub source_time: Option<i64>,
    /// Whether graphics settings files were included (decides which files a restore may remove)
    #[serde(default)]
    pub include_config: Option<bool>,
}

pub(crate) enum SnapReason<'a> {
    Restore(&'a VersionMeta),
    Snapshot(&'a SnapshotMeta),
}

fn snapshot_dir(appid: u32) -> PathBuf {
    data_dir().join("snapshots").join(appid.to_string())
}

/// The zip of snapshot `id`; ids are only letters, digits and '-' (no path tricks).
fn snapshot_path(appid: u32, id: &str) -> Result<PathBuf> {
    let p = snapshot_dir(appid).join(format!("{id}.zip"));
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') || !p.is_file() {
        bail!("{}", tr!("err.snapshot_not_found"));
    }
    Ok(p)
}

fn snapshot_zips(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "zip")).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Details of one snapshot. `with_hash`: hash the files of an old snapshot without metadata
/// (reads the whole zip).
fn read_snapshot(path: &Path, with_hash: bool) -> Result<SnapshotMeta> {
    let id = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut za = zip::ZipArchive::new(File::open(path)?).context(tr!("err.backup_corrupt").to_string())?;
    if let Some(i) = za.index_for_name(SNAP_META) {
        let mut m: SnapshotMeta = serde_json::from_reader(za.by_index(i)?).context(tr!("err.backup_corrupt").to_string())?;
        m.id = id;
        return Ok(m);
    }
    let (mut files, mut bytes) = (0, 0);
    let mut h = Sha256::new();
    for i in 0..za.len() {
        let mut e = za.by_index(i)?;
        if e.is_dir() {
            continue;
        }
        files += 1;
        bytes += e.size();
        if with_hash {
            // Entries are in scan order, as hash_scan reads them
            h.update(e.name().as_bytes());
            h.update([0u8]);
            io::copy(&mut e, &mut h)?;
            h.update([0xffu8]);
        }
    }
    let created_utc = chrono::NaiveDateTime::parse_from_str(id.get(..16).unwrap_or(""), "%Y%m%dT%H%M%SZ")
        .map(|t| t.and_utc().timestamp_millis())
        .or_else(|_| fs::metadata(path)?.modified().map(|m| chrono::DateTime::<chrono::Utc>::from(m).timestamp_millis()))
        .unwrap_or(0);
    Ok(SnapshotMeta {
        id,
        created_utc,
        content_hash: (with_hash && files > 0).then(|| hex::encode(h.finalize())),
        files,
        bytes,
        save_mtime: None,
        reason: String::new(),
        source_device: None,
        source_time: None,
        include_config: None,
    })
}

/// Zip the save on this PC before it is overwritten. A snapshot with the same content is replaced
/// (the new one has the newer date and reason); `protect` is never removed, even when it is the
/// oldest, because it is the snapshot about to be restored.
pub(crate) fn snapshot_local(
    dir: &Path,
    scan: &LocalScan,
    hash: &str,
    reason: SnapReason,
    include_config: bool,
    protect: Option<&Path>,
    on: OnProgress,
) -> Result<PathBuf> {
    fs::create_dir_all(dir)?;
    for p in snapshot_zips(dir) {
        if Some(p.as_path()) != protect && read_snapshot(&p, false).ok().and_then(|m| m.content_hash).as_deref() == Some(hash) {
            let _ = fs::remove_file(&p);
        }
    }
    let now = chrono::Utc::now();
    let stamp = now.format("%Y%m%dT%H%M%SZ").to_string();
    let mut id = stamp.clone();
    let mut n = 1;
    while dir.join(format!("{id}.zip")).exists() {
        n += 1;
        id = format!("{stamp}-{n}");
    }
    let (reason, source_device, source_time) = match reason {
        SnapReason::Restore(v) => ("restore", Some(v.device_name.clone()), Some(v.created_utc)),
        SnapReason::Snapshot(s) => ("snapshot", None, Some(s.save_mtime.unwrap_or(s.created_utc))),
    };
    let meta = SnapshotMeta {
        id: id.clone(),
        created_utc: now.timestamp_millis(),
        content_hash: Some(hash.to_string()),
        files: scan.files.len(),
        bytes: scan.bytes(),
        save_mtime: scan.latest_mtime(),
        reason: reason.into(),
        source_device,
        source_time,
        include_config: Some(include_config),
    };
    // Written under another name first, so a crash never leaves a half-written snapshot
    let path = dir.join(format!("{id}.zip"));
    let part = dir.join(format!("{id}.part"));
    let total = scan.files.len();
    let count = |p: Progress| {
        if let Progress::File { index } = p {
            on(Progress::Count { done: index, total });
        }
    };
    let written = write_zip(&part, scan, Some((SNAP_META, &serde_json::to_vec_pretty(&meta)?)), &count).and_then(|_| Ok(fs::rename(&part, &path)?));
    if written.is_err() {
        let _ = fs::remove_file(&part);
    }
    written?;
    let mut all = snapshot_zips(dir);
    all.retain(|p| Some(p.as_path()) != protect);
    let keep = KEEP_SNAPSHOTS - usize::from(protect.is_some());
    while all.len() > keep {
        let _ = fs::remove_file(all.remove(0));
    }
    Ok(path)
}

/// Snapshots of one game, newest first.
pub fn snapshots(appid: u32) -> Vec<SnapshotMeta> {
    snapshots_in(&snapshot_dir(appid))
}

fn snapshots_in(dir: &Path) -> Vec<SnapshotMeta> {
    let mut v: Vec<SnapshotMeta> = snapshot_zips(dir)
        .iter()
        .filter_map(|p| match read_snapshot(p, true) {
            Ok(m) => Some(m),
            Err(e) => {
                log::warn!("Skipping unreadable snapshot {}: {e:#}", p.display());
                None
            }
        })
        .collect();
    v.sort_by(|a, b| b.created_utc.cmp(&a.created_utc).then(b.id.cmp(&a.id)));
    v
}

pub fn snapshot_count(appid: u32) -> usize {
    snapshot_zips(&snapshot_dir(appid)).len()
}

pub fn snapshot_folder(appid: u32) -> PathBuf {
    snapshot_dir(appid)
}

/// Put snapshot `id` back in place of the save on this PC. The current save is snapshotted first,
/// so this can be undone the same way. Files the snapshot does not have (added later, e.g. by the
/// restore being undone) are removed, so the save is exactly as it was. Last-sync records are not
/// changed: if a cloud backup exists, the game then shows "this PC is newer".
pub fn restore_snapshot(cfg: &Config, appid: u32, id: &str) -> Result<OpResult> {
    let path = snapshot_path(appid, id)?;
    let env = Env::load(cfg)?;
    let g = env.game(appid)?;
    let ctx = env.ctx(g);
    if Procs::snapshot().running(&ctx.base) {
        bail!("{}", tr!("err.game_running", game = g.name));
    }
    let snap = read_snapshot(&path, true)?;
    let Some((restored, removed)) = put_back(&snapshot_dir(appid), &path, &snap, &ctx, env.rules(appid), cfg.include_config)? else {
        return Ok(OpResult { outcome: "unchanged".into(), message: t!("msg.snapshot_unchanged", game = g.name).to_string() });
    };
    let mut message = t!("msg.snapshot_restored", game = g.name, files = restored, time = fmt_time(snap.save_mtime.unwrap_or(snap.created_utc))).to_string();
    if removed > 0 {
        message.push(' ');
        message.push_str(&t!("msg.snapshot_removed", files = removed));
    }
    Ok(OpResult { outcome: "restored".into(), message })
}

/// Snapshot the current save, then write snapshot `path` in its place. Returns (files written,
/// files removed), or None when the save already matches the snapshot.
fn put_back(dir: &Path, path: &Path, snap: &SnapshotMeta, ctx: &GameCtx, rules: &[FileRule], include_config: bool) -> Result<Option<(usize, usize)>> {
    let scan = paths::scan(ctx, rules, include_config);
    let local_hash = hash_scan(&scan)?;
    if local_hash.is_some() && local_hash == snap.content_hash {
        return Ok(None);
    }
    if let Some(h) = &local_hash {
        snapshot_local(dir, &scan, h, SnapReason::Snapshot(snap), include_config, Some(path), &no_progress).context(tr!("err.snapshot").to_string())?;
    }
    let in_snapshot: std::collections::HashSet<String> = {
        let za = zip::ZipArchive::new(File::open(path)?).context(tr!("err.backup_corrupt").to_string())?;
        za.file_names().map(String::from).collect()
    };
    let (restored, _) = extract(path, ctx, None, &no_progress)?;

    // Only remove files of the kinds the snapshot covered: an old snapshot (no metadata) removes
    // nothing, one without graphics settings files leaves those alone.
    let mut removed = 0;
    if let Some(snap_cfg) = snap.include_config {
        let covered = if snap_cfg == include_config { scan } else { paths::scan(ctx, rules, snap_cfg && include_config) };
        for f in covered.files.iter().filter(|f| !in_snapshot.contains(&f.logical)) {
            match fs::remove_file(&f.real) {
                Ok(()) => removed += 1,
                Err(e) => log::warn!("Could not remove {} (not in the snapshot): {e}", f.real.display()),
            }
        }
    }
    Ok(Some((restored, removed)))
}

pub fn delete_snapshot(appid: u32, id: &str) -> Result<()> {
    let path = snapshot_path(appid, id)?;
    fs::remove_file(&path).with_context(|| tr!("err.write_file", path = path.display()).to_string())
}

fn fmt_time(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
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
            steam_ids: ids,
            snapshots: snapshot_count(g.appid),
        });
    }
    Ok(ListResult {
        games: rows,
        cloud_error,
        steam_root: env.root.to_string_lossy().to_string(),
        manifest_games: env.manifest.as_ref().map(|m| m.games.len()).unwrap_or(0),
        account_aliases: idx.account_aliases,
    })
}

/// One game's status (same rules as `list`), plus the account pair to restore into when the
/// latest backup belongs to an account that was replaced by a kept move (`account_aliases`).
pub struct GameStatus {
    pub name: String,
    pub status: &'static str,
    pub remap: Option<(String, String)>,
    /// What the launch-time question shows when the save conflicts (prompt.rs)
    pub info: ConflictInfo,
}

/// Both sides of a game's save, for the question asked when a game starts with a conflict.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictInfo {
    pub appid: u32,
    pub name: String,
    pub status: String,
    pub local_mtime: Option<i64>,
    pub local_files: usize,
    pub local_bytes: u64,
    pub cloud: Option<VersionMeta>,
}

pub fn game_status(cfg: &Config, appid: u32) -> Result<GameStatus> {
    let env = Env::load(cfg)?;
    let g = env.game(appid)?;
    let ctx = env.ctx(g);
    let rules = env.rules(appid);
    let scan = paths::scan(&ctx, rules, cfg.include_config);
    let local = hash_scan(&scan)?;
    let st = storage::open(&cfg.backend)?;
    let idx = load_index(&*st)?;
    let key = appid.to_string();
    let latest = idx.games.get(&key).and_then(|v| v.first());
    let status = status(local.as_deref(), latest, State::load().games.get(&key), !rules.is_empty());
    let remap = latest.and_then(|v| {
        let ids: Vec<String> = v.steam_ids.iter().filter_map(|i| crate::migrate::Ids::parse(i).ok()).map(|i| i.sid64).collect();
        let [only] = ids.as_slice() else { return None };
        idx.account_aliases.get(only).map(|to| (only.clone(), to.clone()))
    });
    let info = ConflictInfo {
        appid,
        name: g.name.clone(),
        status: status.into(),
        local_mtime: scan.latest_mtime(),
        local_files: scan.files.len(),
        local_bytes: scan.bytes(),
        cloud: latest.cloned(),
    };
    Ok(GameStatus { name: g.name.clone(), status, remap, info })
}

/// Where the game is installed, to tell whether it is still running.
pub fn game_base(cfg: &Config, appid: u32) -> Result<PathBuf> {
    let env = Env::load(cfg)?;
    let g = env.game(appid)?;
    Ok(env.ctx(g).base)
}

pub fn backup(cfg: &Config, appid: u32, force: bool, on: OnProgress) -> Result<OpResult> {
    let env = Env::load(cfg)?;
    let g = env.game(appid)?;
    let ctx = env.ctx(g);
    if Procs::snapshot().running(&ctx.base) {
        bail!("{}", tr!("err.game_running", game = g.name));
    }
    let scan = paths::scan(&ctx, env.rules(appid), cfg.include_config);
    let Some(hash) = hash_scan(&scan)? else {
        bail!("{}", tr!("err.no_save_files", game = g.name));
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
            return Ok(OpResult { outcome: "unchanged".into(), message: t!("msg.backup_unchanged", game = g.name).to_string() });
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
        kept: None,
    };
    let tmp = cache_dir().join("tmp").join(format!("{id}.zip"));
    on(Progress::Step { step: "pack" });
    on(Progress::Files { files: scan.files.iter().map(|f| ProgressFile::new(&f.real, f.size)).collect() });
    write_zip(&tmp, &scan, Some((META_IN_ZIP, &serde_json::to_vec_pretty(&meta)?)), on)?;
    on(Progress::Step { step: "upload" });
    let up = st.upload(&tmp, &zip_rel, &|done, total| on(Progress::Bytes { done, total }));
    let _ = fs::remove_file(&tmp);
    up?;

    // Re-read the index right before writing to avoid clobbering another PC's update
    let mut idx = load_index(&*st)?;
    let list = idx.games.entry(key.clone()).or_default();
    // Overwriting progress this PC doesn't have: that cloud version is kept until the user lets
    // it go in History, so the other PC's progress can't age out of the version list
    if let (true, Some(old)) = (force && matches!(s, "cloud" | "conflict" | "diverged"), &latest) {
        if let Some(v) = list.iter_mut().find(|v| v.id == old.id && v.kept.is_none()) {
            log::info!("Keeping cloud version {} of appid={appid}: replaced by a forced backup ({s})", v.id);
            v.kept = Some(Kept { by_device: cfg.device_name.clone(), at_utc: now.timestamp_millis() });
        }
    }
    list.insert(0, meta);
    let pruned = prune(list, cfg.keep_versions);
    save_index(&*st, &idx)?;
    for p in pruned {
        let _ = st.delete(&p.zip);
    }
    state.games.insert(key, SyncBase { local: hash.clone(), cloud: hash, at_utc: now.timestamp_millis() });
    state.save()?;
    Ok(OpResult {
        outcome: "uploaded".into(),
        message: t!("msg.backup_done", game = g.name, files = scan.files.len(), size = human(scan.bytes())).to_string(),
    })
}

/// `remap`: (account in the backup, target account) to write A's save into B's place.
pub fn restore(
    cfg: &Config,
    appid: u32,
    version_id: Option<String>,
    force: bool,
    remap: Option<(String, String)>,
    on: OnProgress,
) -> Result<OpResult> {
    let remap = match remap {
        Some((a, b)) if a != b => Some((crate::migrate::Ids::parse(&a)?, crate::migrate::Ids::parse(&b)?)),
        _ => None,
    };
    let env = Env::load(cfg)?;
    let g = env.game(appid)?;
    let ctx = env.ctx(g);
    if Procs::snapshot().running(&ctx.base) {
        bail!("{}", tr!("err.game_running", game = g.name));
    }
    let rules = env.rules(appid);
    let scan = paths::scan(&ctx, rules, cfg.include_config);
    let local_hash = hash_scan(&scan)?;

    let st = storage::open(&cfg.backend)?;
    let idx = load_index(&*st)?;
    let key = appid.to_string();
    let versions = idx.games.get(&key).filter(|v| !v.is_empty()).context(tr!("err.no_cloud_backup").to_string())?;
    let latest = versions[0].clone();
    let ver = match &version_id {
        Some(id) => versions.iter().find(|v| &v.id == id).cloned().context(tr!("err.version_not_found").to_string())?,
        None => latest.clone(),
    };
    let mut state = State::load();
    let now = chrono::Utc::now().timestamp_millis();

    // Account that must own shared save folders after the restore (see fix_autocloud_owners)
    let owner = match &remap {
        Some((_, to)) => Some(to.clone()),
        None => this_pc_account(&ctx.steam_root),
    };

    // Writing into another account must run even if the source account's save is identical
    if remap.is_none() && local_hash.as_deref() == Some(ver.content_hash.as_str()) {
        let mut local = ver.content_hash.clone();
        if let Some(o) = &owner {
            if fix_autocloud_owners(&scan, o)? > 0 {
                local = hash_scan(&paths::scan(&ctx, rules, cfg.include_config))?.unwrap_or_default();
            }
        }
        state.games.insert(key, SyncBase { local, cloud: latest.content_hash, at_utc: now });
        state.save()?;
        return Ok(OpResult { outcome: "unchanged".into(), message: t!("msg.restore_unchanged", game = g.name).to_string() });
    }
    let s = status(local_hash.as_deref(), Some(&latest), state.games.get(&key), true);
    if !force && local_hash.is_some() && matches!(s, "local" | "conflict" | "diverged") {
        bail!("NEEDS_CONFIRM:{s}");
    }

    let step = |step, files, bytes| StepInfo { step, files, bytes, device: None, time: None };
    let mut steps = Vec::new();
    if local_hash.is_some() {
        steps.push(step("snapshot", scan.files.len(), scan.bytes()));
    }
    steps.push(StepInfo { device: Some(ver.device_name.clone()), time: Some(ver.created_utc), ..step("download", ver.files, 0) });
    steps.push(step("extract", ver.files, ver.bytes));
    on(Progress::Steps { steps });
    if let Some(h) = &local_hash {
        on(Progress::Step { step: "snapshot" });
        snapshot_local(&snapshot_dir(appid), &scan, h, SnapReason::Restore(&ver), cfg.include_config, None, on).context(tr!("err.snapshot").to_string())?;
    }

    let tmp = cache_dir().join("tmp").join(format!("restore-{}.zip", ver.id));
    if let Some(p) = tmp.parent() {
        fs::create_dir_all(p)?;
    }
    on(Progress::Step { step: "download" });
    st.download(&ver.zip, &tmp, &|done, total| on(Progress::Bytes { done, total }))?;
    on(Progress::Step { step: "extract" });
    let result = extract(&tmp, &ctx, remap.as_ref(), on);
    let _ = fs::remove_file(&tmp);
    let (restored, skipped) = result?;

    let mut after_scan = paths::scan(&ctx, rules, cfg.include_config);
    if let Some(o) = &owner {
        if fix_autocloud_owners(&after_scan, o)? > 0 {
            after_scan = paths::scan(&ctx, rules, cfg.include_config);
        }
    }
    let after = hash_scan(&after_scan)?.unwrap_or_default();
    state.games.insert(key, SyncBase { local: after, cloud: latest.content_hash, at_utc: now });
    state.save()?;
    let mut message = t!("msg.restore_done", game = g.name, files = restored, device = ver.device_name, time = fmt_time(ver.created_utc)).to_string();
    if skipped > 0 {
        message.push(' ');
        message.push_str(&t!("msg.restore_skipped_linux", files = skipped));
    }
    if let Some((_, to)) = &remap {
        message.push(' ');
        message.push_str(&t!("msg.restore_into_account", account = to.sid64));
    }
    Ok(OpResult { outcome: "restored".into(), message })
}

/// The Steam account playing on this PC: the one marked most recent in loginusers.vdf,
/// or the only account when there is just one.
fn this_pc_account(steam_root: &Path) -> Option<crate::migrate::Ids> {
    let accts = steam::accounts(steam_root);
    let a = accts.iter().find(|a| a.most_recent).or(if accts.len() == 1 { accts.first() } else { None })?;
    crate::migrate::Ids::parse(&a.steamid64).ok()
}

/// Steam Auto-Cloud marks each save folder with its owner in steam_autocloud.vdf, and clears
/// folders owned by another account when a game starts. A backup made on another PC with another
/// account therefore vanishes on first launch unless the marker names the account on this PC.
/// Folders named after an account (e.g. `Saved/<SteamID64>/`) keep their owner: that is the
/// account the save belongs to, and moving it is the remap's job. Returns how many files changed.
pub(crate) fn fix_autocloud_owners(scan: &LocalScan, owner: &crate::migrate::Ids) -> Result<usize> {
    let mut changed = 0;
    for f in scan.files.iter().filter(|f| crate::migrate::is_autocloud_vdf(&f.real)) {
        let data = fs::read(&f.real)?;
        let Some(cur) = crate::migrate::autocloud_account(&data) else { continue };
        if cur == owner.acct {
            continue;
        }
        let Ok(cur_ids) = crate::migrate::Ids::parse(&cur) else { continue };
        let in_account_folder = f.logical.split('/').skip(2).any(|p| p == cur_ids.acct || p.contains(&cur_ids.sid64));
        if in_account_folder {
            continue;
        }
        fs::write(&f.real, crate::migrate::set_autocloud_account(&data, &owner.acct))
            .with_context(|| tr!("err.write_file", path = f.real.display()).to_string())?;
        changed += 1;
    }
    if changed > 0 {
        log::info!("Gave {changed} shared save folder(s) to the Steam account on this PC (steam_autocloud.vdf)");
    }
    Ok(changed)
}

pub(crate) fn extract(
    zip_path: &Path,
    ctx: &GameCtx,
    remap: Option<&(crate::migrate::Ids, crate::migrate::Ids)>,
    on: OnProgress,
) -> Result<(usize, usize)> {
    let mut za = zip::ZipArchive::new(File::open(zip_path)?).context(tr!("err.backup_corrupt").to_string())?;
    // Validate every path before writing any file
    let mut plan = Vec::new();
    let mut skipped = 0;
    for i in 0..za.len() {
        let e = za.by_index(i)?;
        let name = e.name().to_string();
        if e.is_dir() || name == META_IN_ZIP || name == SNAP_META {
            continue;
        }
        let (logical, rewrite_vdf) = match remap {
            Some((from, to)) => match crate::migrate::remap_logical(&name, from, to) {
                Some(n) => (n, true),
                None => (name.clone(), false),
            },
            None => (name.clone(), false),
        };
        match paths::logical_to_real(&logical, ctx) {
            Ok(p) => plan.push((i, p, rewrite_vdf, e.size())),
            Err(err) if err.to_string() == "SKIP_LINUX_NATIVE" => skipped += 1,
            Err(err) => return Err(err),
        }
    }
    on(Progress::Files { files: plan.iter().map(|(_, p, _, size)| ProgressFile::new(p, *size)).collect() });
    for (index, (i, dest, rewrite_vdf, _)) in plan.iter().enumerate() {
        on(Progress::File { index });
        let mut e = za.by_index(*i)?;
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        match (remap, *rewrite_vdf && crate::migrate::is_autocloud_vdf(dest)) {
            (Some((from, to)), true) => {
                let mut buf = Vec::new();
                io::Read::read_to_end(&mut e, &mut buf)?;
                fs::write(dest, crate::migrate::rewrite_autocloud(&buf, from, to))
                    .with_context(|| tr!("err.write_file", path = dest.display()).to_string())?;
            }
            _ => {
                let mut out = File::create(dest).with_context(|| tr!("err.write_file", path = dest.display()).to_string())?;
                io::copy(&mut e, &mut out)?;
            }
        }
    }
    Ok((plan.len(), skipped))
}

pub fn versions(cfg: &Config, appid: u32) -> Result<Vec<VersionMeta>> {
    let st = storage::open(&cfg.backend)?;
    Ok(load_index(&*st)?.games.remove(&appid.to_string()).unwrap_or_default())
}

/// Stop keeping a version (History): it counts towards `keep_versions` again, so it may be removed
/// right away if there are newer ones.
pub fn unkeep(cfg: &Config, appid: u32, id: &str) -> Result<OpResult> {
    let st = storage::open(&cfg.backend)?;
    let mut idx = load_index(&*st)?;
    let list = idx.games.get_mut(&appid.to_string()).context(tr!("err.version_not_found").to_string())?;
    let v = list.iter_mut().find(|v| v.id == id).context(tr!("err.version_not_found").to_string())?;
    v.kept = None;
    let name = v.game_name.clone();
    let pruned = prune(list, cfg.keep_versions);
    save_index(&*st, &idx)?;
    let removed = pruned.iter().any(|p| p.id == id);
    for p in pruned {
        let _ = st.delete(&p.zip);
    }
    Ok(OpResult {
        outcome: if removed { "removed" } else { "unkept" }.into(),
        message: if removed { t!("msg.unkeep_removed", game = name) } else { t!("msg.unkeep_done", game = name) }.to_string(),
    })
}

pub fn history(cfg: &Config) -> Result<Vec<VersionMeta>> {
    let st = storage::open(&cfg.backend)?;
    let mut all: Vec<VersionMeta> = load_index(&*st)?.games.into_values().flatten().collect();
    all.sort_by(|a, b| b.created_utc.cmp(&a.created_utc));
    all.truncate(100);
    Ok(all)
}

#[derive(Debug, Serialize)]
pub struct DeleteResult {
    pub games: usize,
    pub versions: usize,
    pub message: String,
}

/// Remove every cloud version of one game. The index is updated first, so other PCs never see
/// versions whose zip is gone; then the game's folder is removed. Local saves are not touched;
/// this PC forgets its last-sync record so the game shows as "not backed up".
pub fn delete_cloud_game(cfg: &Config, appid: u32) -> Result<DeleteResult> {
    let st = storage::open(&cfg.backend)?;
    let mut idx = load_index(&*st)?;
    let key = appid.to_string();
    let removed = idx.games.remove(&key).unwrap_or_default();
    let name = removed.first().map(|v| v.game_name.clone()).unwrap_or_else(|| key.clone());
    if !removed.is_empty() {
        save_index(&*st, &idx)?;
    }
    st.delete_dir(&format!("games/{appid}"))?;
    let mut state = State::load();
    if state.games.remove(&key).is_some() {
        state.save()?;
    }
    Ok(DeleteResult {
        games: usize::from(!removed.is_empty()),
        versions: removed.len(),
        message: t!("msg.cloud_game_deleted", game = name, n = removed.len()).to_string(),
    })
}

/// Remove every backup of every game, and the index with the remembered account pairs.
pub fn delete_cloud_all(cfg: &Config) -> Result<DeleteResult> {
    let st = storage::open(&cfg.backend)?;
    let idx = load_index(&*st)?;
    let games = idx.games.values().filter(|v| !v.is_empty()).count();
    let versions = idx.games.values().map(Vec::len).sum();
    // Index first: a half-done delete then leaves orphan zips, never an index pointing at nothing
    st.delete(INDEX)?;
    st.delete_dir("games")?;
    let mut state = State::load();
    if !state.games.is_empty() {
        state.games.clear();
        state.save()?;
    }
    Ok(DeleteResult { games, versions, message: t!("msg.cloud_all_deleted", games = games, n = versions).to_string() })
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

    #[test]
    fn launch_wrappers_are_not_the_game() {
        let v = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let game = "D:\\SteamLibrary\\steamapps\\common\\PBZ\\steam\\PBZ.exe";
        let exe = "C:\\Users\\A\\AppData\\Local\\Checkpoint\\checkpoint.exe";
        assert!(is_launch_wrapper(exe, &v(&[exe, "run", "--appid", "4115450", "--", game])));
        assert!(is_launch_wrapper("/tmp/.mount_Chk/usr/bin/checkpoint", &v(&["/home/u/Apps/Checkpoint_0.7.0_amd64.AppImage", "run", "--", "reaper"])));
        assert!(!is_launch_wrapper(exe, &v(&[exe])));
        assert!(!is_launch_wrapper(game, &v(&[game, "run"])));
        // The current process is skipped: the test binary's own folder does not count as running
        let me = std::env::current_exe().unwrap();
        assert!(!Procs::snapshot().running(me.parent().unwrap()));
    }

    fn meta(h: &str) -> VersionMeta {
        VersionMeta {
            id: "x".into(), appid: 1, game_name: "g".into(), device_id: "d".into(), device_name: "d".into(),
            os: "windows".into(), created_utc: 0, content_hash: h.into(), files: 1, bytes: 1, zip: "z".into(), steam_ids: vec![],
            kept: None,
        }
    }
    fn base(l: &str, c: &str) -> SyncBase {
        SyncBase { local: l.into(), cloud: c.into(), at_utc: 0 }
    }

    /// Runs on the real machine: `cargo test smoke -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn smoke_real_machine() {
        if manifest().is_none() {
            set_manifest(manifest::download_and_build().expect("download manifest"));
        }
        let state_path = crate::config::config_dir().join("state.json");
        let had_state = state_path.exists();
        let tmp = std::env::temp_dir().join("checkpoint-smoke");
        let _ = fs::remove_dir_all(&tmp);
        let mut cfg = Config::load();
        cfg.backend = crate::config::Backend::Folder { path: tmp.to_string_lossy().to_string() };

        let l = list(&cfg).expect("list");
        println!("steam root: {} · manifest {} games", l.steam_root, l.manifest_games);
        for g in &l.games {
            println!("{:>8} {:<40} {:<9} {:>4} files {:>10} B  {}", g.appid, g.name, g.status, g.files, g.bytes, g.roots.join(" | "));
        }
        let Some(g) = l.games.iter().filter(|g| g.local_hash.is_some() && !g.running).min_by_key(|g| g.bytes) else {
            println!("No game with saves to test a backup");
            return;
        };
        let r = backup(&cfg, g.appid, false, &no_progress).expect("backup");
        println!("backup: {}", r.message);
        let again = backup(&cfg, g.appid, false, &no_progress).expect("second backup");
        assert_eq!(again.outcome, "unchanged");
        let l2 = list(&cfg).expect("list 2");
        let row = l2.games.iter().find(|x| x.appid == g.appid).unwrap();
        assert_eq!(row.status, "sync");
        println!("after backup: {} = {}", row.name, row.status);
        if !had_state {
            let _ = fs::remove_file(&state_path);
        } else {
            let mut st = State::load();
            st.games.remove(&g.appid.to_string());
            st.save().unwrap();
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    /// Real backup to the configured storage: `cargo test drive_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn drive_real() {
        let cfg = Config::load();
        let l = list(&cfg).expect("list");
        assert!(l.cloud_error.is_none(), "cloud error: {:?}", l.cloud_error);
        let g = l.games.iter().filter(|g| g.local_hash.is_some() && !g.running).min_by_key(|g| g.bytes).expect("a game with saves");
        println!("game: {} ({} files, {} B), status before: {}", g.name, g.files, g.bytes, g.status);
        let r = backup(&cfg, g.appid, false, &no_progress).expect("backup");
        println!("backup: {} -> {}", r.outcome, r.message);
        let l2 = list(&cfg).expect("list 2");
        let row = l2.games.iter().find(|x| x.appid == g.appid).unwrap();
        println!("after backup: {}, {} versions in the cloud", row.status, row.versions);
        assert_eq!(row.status, "sync");

        let v = versions(&cfg, g.appid).expect("versions");
        let st = storage::open(&cfg.backend).unwrap();
        let tmp = std::env::temp_dir().join("checkpoint-drive-check.zip");
        st.download(&v[0].zip, &tmp, &|_, _| {}).expect("download");
        let za = zip::ZipArchive::new(File::open(&tmp).unwrap()).unwrap();
        let names: Vec<String> = za.file_names().map(String::from).collect();
        println!("downloaded zip: {} entries: {:?}", names.len(), names);
        assert!(names.iter().any(|n| n == META_IN_ZIP));
        assert_eq!(names.len(), v[0].files + 1);
        let _ = fs::remove_file(&tmp);

        let rr = restore(&cfg, g.appid, None, false, None, &no_progress).expect("restore");
        println!("restore of the same version: {} -> {}", rr.outcome, rr.message);
        crate::rcd::shutdown();
        assert_eq!(rr.outcome, "unchanged");
    }

    /// Upload and download progress against the configured storage, with a throwaway file in
    /// `tmp/`: `cargo test progress_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn progress_real() {
        let cfg = Config::load();
        let st = storage::open(&cfg.backend).unwrap();
        let dir = std::env::temp_dir().join(format!("checkpoint-progress-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let src = dir.join("up.bin");
        let data: Vec<u8> = (0..3_000_000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).collect();
        fs::write(&src, &data).unwrap();
        let seen = Mutex::new(Vec::new());
        let on = |d: u64, t: u64| seen.lock().unwrap().push((d, t));
        st.upload(&src, "tmp/progress-test.bin", &on).expect("upload");
        let up = std::mem::take(&mut *seen.lock().unwrap());
        println!("upload: {up:?}");
        st.download("tmp/progress-test.bin", &dir.join("down.bin"), &on).expect("download");
        let down = std::mem::take(&mut *seen.lock().unwrap());
        println!("download: {down:?}");
        let _ = st.delete_dir("tmp");
        crate::rcd::shutdown();
        assert_eq!(fs::read(dir.join("down.bin")).unwrap(), data);
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(up.last(), Some(&(3_000_000, 3_000_000)));
        assert_eq!(down.last(), Some(&(3_000_000, 3_000_000)));
    }

    #[test]
    fn prune_skips_kept_versions() {
        let kept = |id: &str| VersionMeta { kept: Some(Kept { by_device: "PC".into(), at_utc: 1 }), ..meta(id) };
        // newest first: a, b(kept), c, d(kept), e
        let mut list = vec![meta("a"), kept("b"), meta("c"), kept("d"), meta("e")];
        list.iter_mut().zip(["a", "b", "c", "d", "e"]).for_each(|(v, id)| v.id = id.into());
        let removed = prune(&mut list, 2);
        assert_eq!(removed.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), ["e"]);
        assert_eq!(list.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), ["a", "b", "c", "d"]);
        // Old index.json without the field still reads, and writes no "kept" key
        let v: VersionMeta = serde_json::from_str(&serde_json::to_string(&meta("x")).unwrap()).unwrap();
        assert!(v.kept.is_none());
        assert!(!serde_json::to_string(&v).unwrap().contains("kept"));
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
        write_zip(&zip, &scan, None, &no_progress).unwrap();

        fs::write(saves.join("a.sav"), b"progress-2-newer").unwrap();
        let h2 = hash_scan(&paths::scan(&ctx, &rules, false)).unwrap().unwrap();
        assert_ne!(h1, h2);

        let (n, skipped) = extract(&zip, &ctx, None, &no_progress).unwrap();
        assert_eq!((n, skipped), (1, 0));
        assert_eq!(fs::read(saves.join("a.sav")).unwrap(), b"progress-1");
        assert_eq!(hash_scan(&paths::scan(&ctx, &rules, false)).unwrap().unwrap(), h1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn restore_with_remap_writes_into_other_account() {
        let root = std::env::temp_dir().join(format!("checkpoint-remap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let lib = root.join("lib");
        let a = "76561198000000101";
        let b = "76561198000000202";
        let a_dir = lib.join(format!("steamapps/common/G/Saved/{a}"));
        fs::create_dir_all(&a_dir).unwrap();
        fs::write(a_dir.join("s.sav"), b"prog").unwrap();
        let acct_a = crate::migrate::Ids::parse(a).unwrap().acct;
        fs::write(a_dir.join("steam_autocloud.vdf"), format!("\"accountid\" \"{acct_a}\"")).unwrap();
        let ctx = GameCtx::new(9, "G", &lib, &root.join("steam"));
        let rules = vec![FileRule { path: "<base>/Saved".into(), tags: vec![], when: vec![] }];
        let zip = root.join("v.zip");
        write_zip(&zip, &paths::scan(&ctx, &rules, false), None, &no_progress).unwrap();
        fs::remove_dir_all(&a_dir).unwrap();

        let remap = (crate::migrate::Ids::parse(a).unwrap(), crate::migrate::Ids::parse(b).unwrap());
        extract(&zip, &ctx, Some(&remap), &no_progress).unwrap();
        let b_dir = lib.join(format!("steamapps/common/G/Saved/{b}"));
        assert_eq!(fs::read(b_dir.join("s.sav")).unwrap(), b"prog");
        assert!(fs::read_to_string(b_dir.join("steam_autocloud.vdf")).unwrap().contains(&remap.1.acct));
        assert!(!a_dir.exists());
        let _ = fs::remove_dir_all(&root);
    }

    /// Issue #2: a backup from account A restored on a PC signed in as B must not stay owned by A,
    /// or Steam clears the folder when the game starts.
    #[test]
    fn restore_gives_shared_folders_to_this_pc_account() {
        let root = std::env::temp_dir().join(format!("checkpoint-owner-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let lib = root.join("lib");
        let a = crate::migrate::Ids::parse("76561198000000101").unwrap();
        let b = crate::migrate::Ids::parse("76561198000000202").unwrap();
        let vdf = |acct: &str| format!("\"steam_autocloud.vdf\"\n{{\n\t\"accountid\"\t\t\"{acct}\"\n}}\n");
        // Shared install-folder saves (Slay the Spire) and a folder named after account A
        let shared = lib.join("steamapps/common/G/saves");
        let own = lib.join(format!("steamapps/common/G/Saved/{}", a.sid64));
        fs::create_dir_all(&shared).unwrap();
        fs::create_dir_all(&own).unwrap();
        fs::write(shared.join("IRONCLAD.autosave"), b"run").unwrap();
        fs::write(shared.join("steam_autocloud.vdf"), vdf(&a.acct)).unwrap();
        fs::write(own.join("steam_autocloud.vdf"), vdf(&a.acct)).unwrap();
        let ctx = GameCtx::new(646570, "G", &lib, &root.join("steam"));
        let rules = vec![
            FileRule { path: "<base>/saves".into(), tags: vec!["save".into()], when: vec![] },
            FileRule { path: "<base>/Saved".into(), tags: vec!["save".into()], when: vec![] },
        ];
        let zip = root.join("v.zip");
        write_zip(&zip, &paths::scan(&ctx, &rules, false), None, &no_progress).unwrap();
        fs::remove_dir_all(lib.join("steamapps/common/G")).unwrap();

        extract(&zip, &ctx, None, &no_progress).unwrap();
        let scan = paths::scan(&ctx, &rules, false);
        assert_eq!(fix_autocloud_owners(&scan, &b).unwrap(), 1);
        let shared_vdf = fs::read(shared.join("steam_autocloud.vdf")).unwrap();
        assert_eq!(crate::migrate::autocloud_account(&shared_vdf).as_deref(), Some(b.acct.as_str()));
        assert_eq!(String::from_utf8(shared_vdf).unwrap(), vdf(&b.acct)); // rest of the file untouched
        let own_vdf = fs::read(own.join("steam_autocloud.vdf")).unwrap();
        assert_eq!(crate::migrate::autocloud_account(&own_vdf).as_deref(), Some(a.acct.as_str()));
        assert_eq!(fs::read(shared.join("IRONCLAD.autosave")).unwrap(), b"run");
        // Running it again changes nothing
        assert_eq!(fix_autocloud_owners(&paths::scan(&ctx, &rules, false), &b).unwrap(), 0);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn snapshot_put_back_is_exact_and_undoable() {
        let root = std::env::temp_dir().join(format!("checkpoint-snap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let lib = root.join("lib");
        let dir = root.join("snapshots");
        let saves = lib.join("steamapps/common/G/saves");
        fs::create_dir_all(&saves).unwrap();
        fs::write(saves.join("a.sav"), b"mine-level-10").unwrap();
        let ctx = GameCtx::new(7, "G", &lib, &root.join("steam"));
        let rules = vec![FileRule { path: "<base>/saves".into(), tags: vec!["save".into()], when: vec![] }];
        let hash_now = || hash_scan(&paths::scan(&ctx, &rules, false)).unwrap().unwrap();
        let mine = hash_now();

        // A cloud restore overwrites a.sav and adds b.sav; the snapshot keeps the save it replaced
        let cloud = VersionMeta { device_name: "Jonsbo".into(), created_utc: 5, ..meta("c") };
        let p1 = snapshot_local(&dir, &paths::scan(&ctx, &rules, false), &mine, SnapReason::Restore(&cloud), false, None, &no_progress).unwrap();
        fs::write(saves.join("a.sav"), b"cloud-level-5").unwrap();
        fs::write(saves.join("b.sav"), b"cloud-extra").unwrap();
        let theirs = hash_now();

        let list = snapshots_in(&dir);
        assert_eq!(list.len(), 1);
        let s = &list[0];
        assert_eq!((s.reason.as_str(), s.source_device.as_deref(), s.source_time), ("restore", Some("Jonsbo"), Some(5)));
        assert_eq!((s.files, s.content_hash.as_deref()), (1, Some(mine.as_str())));

        // Going back: exactly the old save, and the cloud one is kept as a new snapshot
        assert_eq!(put_back(&dir, &p1, s, &ctx, &rules, false).unwrap(), Some((1, 1)));
        assert_eq!(fs::read(saves.join("a.sav")).unwrap(), b"mine-level-10");
        assert!(!saves.join("b.sav").exists());
        assert_eq!(hash_now(), mine);
        let list = snapshots_in(&dir);
        assert_eq!(list.len(), 2);
        assert_eq!((list[0].reason.as_str(), list[0].content_hash.as_deref()), ("snapshot", Some(theirs.as_str())));
        assert_eq!(put_back(&dir, &p1, &list[1], &ctx, &rules, false).unwrap(), None, "already the same");

        // Undoing the undo brings the cloud save back, b.sav included
        let p2 = dir.join(format!("{}.zip", list[0].id));
        assert_eq!(put_back(&dir, &p2, &list[0], &ctx, &rules, false).unwrap(), Some((2, 0)));
        assert_eq!(hash_now(), theirs);
        // The save it replaced (mine) was already kept: replaced by a newer copy, not duplicated
        let list = snapshots_in(&dir);
        assert_eq!(list.len(), 2);
        assert_eq!(list.iter().filter(|m| m.content_hash.as_deref() == Some(mine.as_str())).count(), 1);
        assert_eq!(list[0].content_hash.as_deref(), Some(mine.as_str()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn snapshots_are_pruned_but_the_one_restored_stays() {
        let root = std::env::temp_dir().join(format!("checkpoint-prune-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let lib = root.join("lib");
        let dir = root.join("snapshots");
        let saves = lib.join("steamapps/common/G/saves");
        fs::create_dir_all(&saves).unwrap();
        let ctx = GameCtx::new(7, "G", &lib, &root.join("steam"));
        let rules = vec![FileRule { path: "<base>/saves".into(), tags: vec![], when: vec![] }];
        let mut first = None;
        for i in 0..KEEP_SNAPSHOTS + 3 {
            fs::write(saves.join("a.sav"), format!("v{i}")).unwrap();
            let scan = paths::scan(&ctx, &rules, false);
            let h = hash_scan(&scan).unwrap().unwrap();
            let p = snapshot_local(&dir, &scan, &h, SnapReason::Restore(&meta("c")), false, first.as_deref(), &no_progress).unwrap();
            first.get_or_insert(p);
        }
        let zips = snapshot_zips(&dir);
        assert_eq!(zips.len(), KEEP_SNAPSHOTS);
        assert!(zips.contains(first.as_ref().unwrap()), "the protected (oldest) snapshot was removed");
        assert!(fs::read_dir(&dir).unwrap().flatten().all(|e| e.path().extension().is_some_and(|x| x == "zip")), "no leftover .part files");
        let _ = fs::remove_dir_all(&root);
    }

    /// Snapshots from before 0.7 are plain zips: their details come from the zip itself, and
    /// putting one back never removes files (it is unknown which kinds it covered).
    #[test]
    fn old_snapshots_without_metadata_still_work() {
        let root = std::env::temp_dir().join(format!("checkpoint-oldsnap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let lib = root.join("lib");
        let dir = root.join("snapshots");
        let saves = lib.join("steamapps/common/G/saves");
        fs::create_dir_all(&saves).unwrap();
        fs::write(saves.join("a.sav"), b"old").unwrap();
        let ctx = GameCtx::new(7, "G", &lib, &root.join("steam"));
        let rules = vec![FileRule { path: "<base>/saves".into(), tags: vec![], when: vec![] }];
        let scan = paths::scan(&ctx, &rules, false);
        let old = hash_scan(&scan).unwrap().unwrap();
        let p = dir.join("20250102T030405Z.zip");
        write_zip(&p, &scan, None, &no_progress).unwrap();
        fs::write(saves.join("a.sav"), b"new").unwrap();
        fs::write(saves.join("b.sav"), b"extra").unwrap();

        let m = read_snapshot(&p, true).unwrap();
        assert_eq!((m.files, m.content_hash.as_deref(), m.reason.as_str()), (1, Some(old.as_str()), ""));
        assert_eq!(m.created_utc, chrono::DateTime::parse_from_rfc3339("2025-01-02T03:04:05Z").unwrap().timestamp_millis());
        assert_eq!(put_back(&dir, &p, &m, &ctx, &rules, false).unwrap(), Some((1, 0)));
        assert_eq!(fs::read(saves.join("a.sav")).unwrap(), b"old");
        assert!(saves.join("b.sav").exists());
        assert!(snapshot_path(7, "../evil").is_err());
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
