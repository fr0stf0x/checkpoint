#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod manifest;
mod migrate;
mod paths;
mod rclone;
mod steam;
mod storage;
mod sync;
mod vdf;

use config::{Backend, Config};
use serde::Serialize;
use std::sync::Mutex;

/// Chỉ cho chạy 1 thao tác backup/restore tại một thời điểm.
static OP: Mutex<()> = Mutex::new(());

async fn blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[derive(Serialize)]
struct Info {
    config: Config,
    os: String,
    steam_root: Option<String>,
    manifest_games: usize,
    manifest_updated: Option<i64>,
    config_dir: String,
}

#[tauri::command]
async fn get_info() -> Result<Info, String> {
    blocking(|| {
        let cfg = Config::load();
        let m = sync::manifest();
        Ok(Info {
            steam_root: steam::find_root(cfg.steam_root_override.as_deref()).map(|p| p.to_string_lossy().to_string()),
            os: if cfg!(windows) { "windows".into() } else { "linux".into() },
            manifest_games: m.as_ref().map(|m| m.games.len()).unwrap_or(0),
            manifest_updated: m.as_ref().map(|m| m.updated_utc),
            config_dir: config::config_dir().to_string_lossy().to_string(),
            config: cfg,
        })
    })
    .await
}

#[tauri::command]
async fn save_config(config: Config) -> Result<(), String> {
    blocking(move || config.save()).await
}

#[tauri::command]
async fn update_manifest() -> Result<usize, String> {
    blocking(|| Ok(sync::set_manifest(manifest::download_and_build()?))).await
}

#[tauri::command]
async fn list_games() -> Result<sync::ListResult, String> {
    blocking(|| sync::list(&Config::load())).await
}

#[tauri::command]
async fn list_accounts() -> Result<Vec<steam::Account>, String> {
    blocking(|| {
        let cfg = Config::load();
        let root = steam::find_root(cfg.steam_root_override.as_deref())
            .ok_or_else(|| anyhow::anyhow!("Không tìm thấy thư mục Steam"))?;
        Ok(steam::accounts(&root))
    })
    .await
}

#[tauri::command]
async fn backup_game(appid: u32, force: bool) -> Result<sync::OpResult, String> {
    blocking(move || {
        let _g = OP.lock().unwrap_or_else(|e| e.into_inner());
        sync::backup(&Config::load(), appid, force)
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
    blocking(move || {
        let _g = OP.lock().unwrap_or_else(|e| e.into_inner());
        let remap = remap_from.zip(remap_to);
        sync::restore(&Config::load(), appid, version_id, force, remap)
    })
    .await
}

#[tauri::command]
async fn migrate_preview(appid: u32, from: String, to: String) -> Result<migrate::Preview, String> {
    blocking(move || migrate::preview(&Config::load(), appid, &from, &to)).await
}

#[tauri::command]
async fn migrate_apply(appid: u32, from: String, to: String, patch_in_file: bool) -> Result<migrate::Migration, String> {
    blocking(move || {
        let _g = OP.lock().unwrap_or_else(|e| e.into_inner());
        migrate::apply(&Config::load(), appid, &from, &to, patch_in_file)
    })
    .await
}

#[tauri::command]
async fn migrate_undo(id: String) -> Result<migrate::Migration, String> {
    blocking(move || {
        let _g = OP.lock().unwrap_or_else(|e| e.into_inner());
        migrate::undo(&Config::load(), &id)
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
    blocking(move || {
        let (migration, warning) = migrate::keep(&Config::load(), &id)?;
        Ok(KeepResult { migration, warning })
    })
    .await
}

#[tauri::command]
async fn migrate_pending() -> Result<Vec<migrate::Migration>, String> {
    blocking(|| Ok(migrate::pending())).await
}

#[tauri::command]
async fn list_versions(appid: u32) -> Result<Vec<sync::VersionMeta>, String> {
    blocking(move || sync::versions(&Config::load(), appid)).await
}

#[tauri::command]
async fn history() -> Result<Vec<sync::VersionMeta>, String> {
    blocking(|| sync::history(&Config::load())).await
}

#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    blocking(move || {
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
    blocking(|| {
        let cfg = Config::load();
        Ok(rclone::status(configured_bin(&cfg).as_deref(), REMOTE_NAME))
    })
    .await
}

/// Tải rclone chính thức về thư mục của app.
#[tauri::command]
async fn rclone_install() -> Result<String, String> {
    blocking(|| {
        let _g = OP.lock().unwrap_or_else(|e| e.into_inner());
        rclone::install()
    })
    .await
}

/// Tạo remote Google Drive bằng client_id riêng rồi trỏ app vào gdrive:Checkpoint.
/// Lệnh này mở trình duyệt để đăng nhập Google.
#[tauri::command]
async fn rclone_connect(client_id: String, client_secret: String) -> Result<String, String> {
    blocking(move || {
        let mut cfg = Config::load();
        let bin = configured_bin(&cfg);
        rclone::connect(bin.as_deref(), REMOTE_NAME, &client_id, &client_secret)?;
        let remote = format!("{REMOTE_NAME}:Checkpoint");
        cfg.backend = Backend::Rclone { remote: remote.clone(), bin };
        cfg.save()?;
        Ok(remote)
    })
    .await
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            rclone_connect
        ])
        .run(tauri::generate_context!())
        .expect("Không khởi động được Checkpoint");
}
