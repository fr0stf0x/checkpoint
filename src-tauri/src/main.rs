#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod logger;
mod manifest;
mod migrate;
mod paths;
mod rclone;
mod report;
mod steam;
mod storage;
mod sync;
mod vdf;

use config::{Backend, Config};
use log::{error, info};
use serde::Serialize;
use std::sync::Mutex;

/// Chỉ cho chạy 1 thao tác backup/restore tại một thời điểm.
static OP: Mutex<()> = Mutex::new(());

/// Chạy việc nặng ngoài luồng giao diện. Lỗi nào cũng được ghi vào log kèm tên thao tác.
async fn blocking<T, F>(label: &'static str, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    let r = tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("{label}: luồng xử lý bị dừng: {e}"))?
        .map_err(|e| format!("{e:#}"));
    if let Err(e) = &r {
        if e.starts_with("NEEDS_CONFIRM") {
            info!("{label}: cần người dùng xác nhận ({e})");
        } else {
            error!("{label} lỗi: {e}");
        }
    }
    r
}

#[derive(Serialize)]
struct Info {
    config: Config,
    os: String,
    steam_root: Option<String>,
    manifest_games: usize,
    manifest_updated: Option<i64>,
    config_dir: String,
    version: String,
    log_dir: String,
}

#[tauri::command]
async fn get_info() -> Result<Info, String> {
    blocking("get_info", || {
        let cfg = Config::load();
        let m = sync::manifest();
        Ok(Info {
            steam_root: steam::find_root(cfg.steam_root_override.as_deref()).map(|p| p.to_string_lossy().to_string()),
            os: if cfg!(windows) { "windows".into() } else { "linux".into() },
            manifest_games: m.as_ref().map(|m| m.games.len()).unwrap_or(0),
            manifest_updated: m.as_ref().map(|m| m.updated_utc),
            config_dir: config::config_dir().to_string_lossy().to_string(),
            version: env!("CARGO_PKG_VERSION").into(),
            log_dir: logger::log_dir().to_string_lossy().to_string(),
            config: cfg,
        })
    })
    .await
}

#[tauri::command]
async fn save_config(config: Config) -> Result<(), String> {
    blocking("save_config", move || {
        let kind = match &config.backend {
            Backend::None => "none",
            Backend::Folder { .. } => "folder",
            Backend::Rclone { .. } => "rclone",
        };
        info!("Lưu cài đặt: nơi lưu={kind}, backup cài đặt={}, giữ {} bản", config.include_config, config.keep_versions);
        config.save()
    })
    .await
}

#[tauri::command]
async fn update_manifest() -> Result<usize, String> {
    blocking("update_manifest", || {
        info!("Tải manifest từ {}", manifest::MANIFEST_URL);
        let n = sync::set_manifest(manifest::download_and_build()?);
        info!("Manifest: {n} game có Steam AppID");
        Ok(n)
    })
    .await
}

#[tauri::command]
async fn list_games() -> Result<sync::ListResult, String> {
    blocking("list_games", || {
        let t = std::time::Instant::now();
        let r = sync::list(&Config::load())?;
        let with_save = r.games.iter().filter(|g| g.local_hash.is_some()).count();
        info!(
            "Quét xong {} game ({with_save} có save) trong {} ms{}",
            r.games.len(),
            t.elapsed().as_millis(),
            r.cloud_error.as_deref().map(|e| format!(", cloud lỗi: {e}")).unwrap_or_default()
        );
        Ok(r)
    })
    .await
}

#[tauri::command]
async fn list_accounts() -> Result<Vec<steam::Account>, String> {
    blocking("list_accounts", || {
        let cfg = Config::load();
        let root = steam::find_root(cfg.steam_root_override.as_deref())
            .ok_or_else(|| anyhow::anyhow!("Không tìm thấy thư mục Steam"))?;
        let a = steam::accounts(&root);
        info!("Đọc được {} tài khoản Steam", a.len());
        Ok(a)
    })
    .await
}

#[tauri::command]
async fn backup_game(appid: u32, force: bool) -> Result<sync::OpResult, String> {
    blocking("backup_game", move || {
        let _g = OP.lock().unwrap_or_else(|e| e.into_inner());
        info!("Backup appid={appid} force={force}");
        let r = sync::backup(&Config::load(), appid, force)?;
        info!("Backup appid={appid}: {} · {}", r.outcome, r.message);
        Ok(r)
    })
    .await
}

#[tauri::command]
async fn restore_game(
    appid: u32,
    version_id: Option<String>,
    force: bool,
    remap_from: Option<String>,
    remap_to: Option<String>,
) -> Result<sync::OpResult, String> {
    blocking("restore_game", move || {
        let _g = OP.lock().unwrap_or_else(|e| e.into_inner());
        info!(
            "Restore appid={appid} version={} force={force}{}",
            version_id.as_deref().unwrap_or("mới nhất"),
            match (&remap_from, &remap_to) {
                (Some(a), Some(b)) => format!(" ghi từ tài khoản {a} sang {b}"),
                _ => String::new(),
            }
        );
        let remap = remap_from.zip(remap_to);
        let r = sync::restore(&Config::load(), appid, version_id, force, remap)?;
        info!("Restore appid={appid}: {} · {}", r.outcome, r.message);
        Ok(r)
    })
    .await
}

#[tauri::command]
async fn migrate_preview(appid: u32, from: String, to: String) -> Result<migrate::Preview, String> {
    blocking("migrate_preview", move || {
        let p = migrate::preview(&Config::load(), appid, &from, &to)?;
        info!("Xem trước chuyển save appid={appid} {from} → {to}: loại {}, {} file, ghi đè {}", p.class, p.files.len(), p.overwrite);
        Ok(p)
    })
    .await
}

#[tauri::command]
async fn migrate_apply(appid: u32, from: String, to: String, patch_in_file: bool) -> Result<migrate::Migration, String> {
    blocking("migrate_apply", move || {
        let _g = OP.lock().unwrap_or_else(|e| e.into_inner());
        info!("Chuyển save appid={appid} {from} → {to} vá_ID_trong_file={patch_in_file}");
        let m = migrate::apply(&Config::load(), appid, &from, &to, patch_in_file)?;
        info!("Chuyển save {}: {} file, vá {} file, chờ xác nhận", m.id, m.files, m.patched);
        Ok(m)
    })
    .await
}

#[tauri::command]
async fn migrate_undo(id: String) -> Result<migrate::Migration, String> {
    blocking("migrate_undo", move || {
        let _g = OP.lock().unwrap_or_else(|e| e.into_inner());
        let m = migrate::undo(&Config::load(), &id)?;
        info!("Hoàn tác chuyển save {id}");
        Ok(m)
    })
    .await
}

#[derive(Serialize)]
struct KeepResult {
    migration: migrate::Migration,
    warning: Option<String>,
}

#[tauri::command]
async fn migrate_keep(id: String) -> Result<KeepResult, String> {
    blocking("migrate_keep", move || {
        let (migration, warning) = migrate::keep(&Config::load(), &id)?;
        info!("Giữ chuyển save {id}{}", warning.as_deref().map(|w| format!(" (cảnh báo: {w})")).unwrap_or_default());
        Ok(KeepResult { migration, warning })
    })
    .await
}

#[tauri::command]
async fn migrate_pending() -> Result<Vec<migrate::Migration>, String> {
    blocking("migrate_pending", || Ok(migrate::pending())).await
}

#[tauri::command]
async fn list_versions(appid: u32) -> Result<Vec<sync::VersionMeta>, String> {
    blocking("list_versions", move || sync::versions(&Config::load(), appid)).await
}

#[tauri::command]
async fn history() -> Result<Vec<sync::VersionMeta>, String> {
    blocking("history", || sync::history(&Config::load())).await
}

#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    blocking("pick_folder", move || {
        Ok(app
            .dialog()
            .file()
            .blocking_pick_folder()
            .and_then(|p| p.into_path().ok())
            .map(|p| p.to_string_lossy().to_string()))
    })
    .await
}

const REMOTE_NAME: &str = "gdrive";

/// rclone mà người dùng chỉ định trong cấu hình (nếu có).
fn configured_bin(cfg: &Config) -> Option<String> {
    match &cfg.backend {
        Backend::Rclone { bin: Some(b), .. } if !b.trim().is_empty() => Some(b.clone()),
        _ => None,
    }
}

#[tauri::command]
async fn rclone_status() -> Result<rclone::RcloneStatus, String> {
    blocking("rclone_status", || {
        let cfg = Config::load();
        let s = rclone::status(configured_bin(&cfg).as_deref(), REMOTE_NAME);
        info!(
            "rclone: {} · remote gdrive: {}",
            s.version.as_deref().unwrap_or("chưa có"),
            match &s.remote {
                None => "chưa có".to_string(),
                Some(r) => format!("{} client riêng={}", r.scope.as_deref().unwrap_or("?"), r.custom_client),
            }
        );
        Ok(s)
    })
    .await
}

/// Tải rclone chính thức về thư mục của app.
#[tauri::command]
async fn rclone_install() -> Result<String, String> {
    blocking("rclone_install", || {
        let _g = OP.lock().unwrap_or_else(|e| e.into_inner());
        info!("Cài rclone");
        let v = rclone::install()?;
        info!("Đã cài {v}");
        Ok(v)
    })
    .await
}

/// Tạo remote Google Drive bằng client_id riêng rồi trỏ app vào gdrive:Checkpoint.
/// Lệnh này mở trình duyệt để đăng nhập Google. Không ghi client secret vào log.
#[tauri::command]
async fn rclone_connect(client_id: String, client_secret: String) -> Result<String, String> {
    blocking("rclone_connect", move || {
        info!("Kết nối Google Drive bằng client ID riêng");
        let mut cfg = Config::load();
        let bin = configured_bin(&cfg);
        rclone::connect(bin.as_deref(), REMOTE_NAME, &client_id, &client_secret)?;
        let remote = format!("{REMOTE_NAME}:Checkpoint");
        cfg.backend = Backend::Rclone { remote: remote.clone(), bin };
        cfg.save()?;
        info!("Đã kết nối {remote}");
        Ok(remote)
    })
    .await
}

// ---------- log và báo lỗi ----------

/// Giao diện gửi lỗi JavaScript và sự kiện quan trọng về để ghi chung một file log.
#[tauri::command]
fn log_frontend(level: String, message: String) {
    let msg: String = message.chars().take(4000).collect();
    match level.as_str() {
        "error" => log::error!(target: "checkpoint::ui", "{msg}"),
        "warn" => log::warn!(target: "checkpoint::ui", "{msg}"),
        _ => log::info!(target: "checkpoint::ui", "{msg}"),
    }
}

#[tauri::command]
async fn get_log(lines: Option<usize>) -> Result<String, String> {
    blocking("get_log", move || Ok(logger::tail(lines.unwrap_or(2000).min(20000)))).await
}

#[tauri::command]
fn open_log_folder(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let _ = std::fs::create_dir_all(logger::log_dir());
    app.opener()
        .open_path(logger::log_dir().to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| format!("Không mở được thư mục log: {e}"))
}

#[derive(Serialize)]
struct ReportResult {
    url: String,
    log_truncated: bool,
}

/// Mở trang tạo issue trên GitHub với nội dung điền sẵn. kind: "bug" | "feature".
#[tauri::command]
fn report_issue(app: tauri::AppHandle, kind: String, title: String, error: Option<String>) -> Result<ReportResult, String> {
    use tauri_plugin_opener::OpenerExt;
    info!("Mở trang báo cáo GitHub: {kind}");
    let link = report::issue_url(&kind, &title, error.as_deref());
    app.opener()
        .open_url(link.url.clone(), None::<&str>)
        .map_err(|e| format!("Không mở được trình duyệt: {e}"))?;
    Ok(ReportResult { url: link.url, log_truncated: link.log_truncated })
}

fn main() {
    logger::init();
    info!("Checkpoint {} khởi động · {}", env!("CARGO_PKG_VERSION"), report::system_line());
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |p| {
        error!("PANIC: {p}");
        log::logger().flush();
        prev(p);
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_info,
            save_config,
            update_manifest,
            list_games,
            list_accounts,
            backup_game,
            restore_game,
            list_versions,
            history,
            pick_folder,
            migrate_preview,
            migrate_apply,
            migrate_undo,
            migrate_keep,
            migrate_pending,
            rclone_status,
            rclone_install,
            rclone_connect,
            log_frontend,
            get_log,
            open_log_folder,
            report_issue
        ])
        .run(tauri::generate_context!())
        .expect("Không khởi động được Checkpoint");
}
