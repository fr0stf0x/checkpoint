#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Translations: src-tauri/locales/<lang>.yml. Backend messages use tr!("err.*"/"msg.*"),
// the UI gets the "ui.*" subtree through get_strings. Logs are always English.
rust_i18n::i18n!("locales", fallback = "en");

mod config;
mod i18n;
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
use std::collections::BTreeMap;
use std::sync::Mutex;

/// Only one backup/restore/move operation at a time.
static OP: Mutex<()> = Mutex::new(());

/// Run heavy work off the UI thread. Every error is logged (in English) with the operation name;
/// the returned error stays in the UI language.
async fn blocking<T, F>(label: &'static str, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        i18n::clear();
        let r = f().map_err(|e| format!("{e:#}"));
        if let Err(e) = &r {
            let en = i18n::to_en(e);
            if e.starts_with("NEEDS_CONFIRM") {
                info!("{label}: needs user confirmation ({en})");
            } else {
                error!("{label} failed: {en}");
            }
        }
        r
    })
    .await
    .map_err(|e| format!("{label}: worker thread stopped: {e}"))?
}

fn apply_language(cfg: &Config) -> String {
    let lang = cfg.effective_language();
    rust_i18n::set_locale(&lang);
    lang
}

// ---------- translations for the UI ----------

const LOCALE_FILES: &[(&str, &str)] = &[("en", include_str!("../locales/en.yml")), ("vi", include_str!("../locales/vi.yml"))];

fn flatten(prefix: &str, v: &serde_yaml::Value, out: &mut BTreeMap<String, String>) {
    match v {
        serde_yaml::Value::Mapping(m) => {
            for (k, val) in m {
                if let Some(k) = k.as_str() {
                    let key = if prefix.is_empty() { k.to_string() } else { format!("{prefix}.{k}") };
                    flatten(&key, val, out);
                }
            }
        }
        serde_yaml::Value::String(s) => {
            out.insert(prefix.to_string(), s.clone());
        }
        other => {
            if let Ok(s) = serde_yaml::to_string(other) {
                out.insert(prefix.to_string(), s.trim().to_string());
            }
        }
    }
}

fn ui_strings(lang: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    // English first, then the chosen language on top, so missing keys fall back to English
    for (_, src) in LOCALE_FILES.iter().filter(|(c, _)| *c == "en" || *c == lang) {
        if let Ok(v) = serde_yaml::from_str::<serde_yaml::Value>(src) {
            if let Some(ui) = v.get("ui") {
                flatten("", ui, &mut out);
            }
        }
    }
    out
}

#[derive(Serialize)]
struct Strings {
    lang: String,
    strings: BTreeMap<String, String>,
}

#[tauri::command]
fn get_strings() -> Strings {
    let lang = apply_language(&Config::load());
    Strings { strings: ui_strings(&lang), lang }
}

// ---------- app info and settings ----------

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
    language: String,
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
            language: cfg.effective_language(),
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
        info!(
            "Settings saved: storage={kind}, include_config={}, keep_versions={}, language={}",
            config.include_config, config.keep_versions, config.language
        );
        config.save()?;
        apply_language(&config);
        Ok(())
    })
    .await
}

#[tauri::command]
async fn update_manifest() -> Result<usize, String> {
    blocking("update_manifest", || {
        info!("Downloading manifest from {}", manifest::MANIFEST_URL);
        let n = sync::set_manifest(manifest::download_and_build()?);
        info!("Manifest: {n} games with a Steam AppID");
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
            "Scanned {} games ({with_save} with saves) in {} ms{}",
            r.games.len(),
            t.elapsed().as_millis(),
            r.cloud_error.as_deref().map(|e| format!(", cloud error: {}", i18n::to_en(e))).unwrap_or_default()
        );
        Ok(r)
    })
    .await
}

#[tauri::command]
async fn list_accounts() -> Result<Vec<steam::Account>, String> {
    blocking("list_accounts", || {
        let cfg = Config::load();
        let root = steam::find_root(cfg.steam_root_override.as_deref()).ok_or_else(|| anyhow::anyhow!("{}", crate::tr!("err.no_steam")))?;
        let a = steam::accounts(&root);
        info!("Found {} Steam accounts", a.len());
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
        info!("Backup appid={appid}: {}", r.outcome);
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
            version_id.as_deref().unwrap_or("latest"),
            match (&remap_from, &remap_to) {
                (Some(a), Some(b)) => format!(" into another account: {a} -> {b}"),
                _ => String::new(),
            }
        );
        let remap = remap_from.zip(remap_to);
        let r = sync::restore(&Config::load(), appid, version_id, force, remap)?;
        info!("Restore appid={appid}: {}", r.outcome);
        Ok(r)
    })
    .await
}

#[tauri::command]
async fn migrate_preview(appid: u32, from: String, to: String) -> Result<migrate::Preview, String> {
    blocking("migrate_preview", move || {
        let p = migrate::preview(&Config::load(), appid, &from, &to)?;
        info!("Move preview appid={appid} {from} -> {to}: class={}, files={}, overwrite={}", p.class, p.files.len(), p.overwrite);
        Ok(p)
    })
    .await
}

#[tauri::command]
async fn migrate_apply(appid: u32, from: String, to: String, patch_in_file: bool) -> Result<migrate::Migration, String> {
    blocking("migrate_apply", move || {
        let _g = OP.lock().unwrap_or_else(|e| e.into_inner());
        info!("Move save appid={appid} {from} -> {to} patch_in_file={patch_in_file}");
        let m = migrate::apply(&Config::load(), appid, &from, &to, patch_in_file)?;
        info!("Move {}: {} files, {} patched, waiting for confirmation", m.id, m.files, m.patched);
        Ok(m)
    })
    .await
}

#[tauri::command]
async fn migrate_undo(id: String) -> Result<migrate::Migration, String> {
    blocking("migrate_undo", move || {
        let _g = OP.lock().unwrap_or_else(|e| e.into_inner());
        let m = migrate::undo(&Config::load(), &id)?;
        info!("Move {id} undone");
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
        info!("Move {id} kept{}", if warning.is_some() { " (account pair not saved to the cloud)" } else { "" });
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

// ---------- rclone / Google Drive ----------

const REMOTE_NAME: &str = "gdrive";

/// rclone path set by the user in Settings, if any.
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
            s.version.as_deref().unwrap_or("not found"),
            match &s.remote {
                None => "none".to_string(),
                Some(r) => format!("scope={} own_client={}", r.scope.as_deref().unwrap_or("?"), r.custom_client),
            }
        );
        Ok(s)
    })
    .await
}

/// Download the official rclone into the app folder.
#[tauri::command]
async fn rclone_install() -> Result<String, String> {
    blocking("rclone_install", || {
        let _g = OP.lock().unwrap_or_else(|e| e.into_inner());
        info!("Installing rclone");
        let v = rclone::install()?;
        info!("Installed {v}");
        Ok(v)
    })
    .await
}

/// Create the Google Drive remote with the user's own client ID and point the app at gdrive:Checkpoint.
/// Opens the browser for Google sign-in. The client secret is never logged.
#[tauri::command]
async fn rclone_connect(client_id: String, client_secret: String) -> Result<String, String> {
    blocking("rclone_connect", move || {
        info!("Connecting Google Drive with own client ID");
        let mut cfg = Config::load();
        let bin = configured_bin(&cfg);
        rclone::connect(bin.as_deref(), REMOTE_NAME, &client_id, &client_secret)?;
        let remote = format!("{REMOTE_NAME}:Checkpoint");
        cfg.backend = Backend::Rclone { remote: remote.clone(), bin };
        cfg.save()?;
        info!("Connected {remote}");
        Ok(remote)
    })
    .await
}

/// The Google sign-in link of the connect in progress (None until rclone prints it).
#[tauri::command]
fn rclone_auth_url() -> Option<String> {
    rclone::auth_url()
}

/// Open the sign-in link of the connect in progress in the default browser.
/// Only the link captured from rclone can be opened, never a URL from the UI.
#[tauri::command]
fn open_auth_url(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let url = rclone::auth_url().ok_or_else(|| crate::tr!("err.no_auth_url").to_string())?;
    info!("Opening Google sign-in link in the browser");
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| crate::tr!("err.open_browser", detail = e.to_string()).to_string())
}

// ---------- logs and bug reports ----------

/// The UI sends JavaScript errors and notable events here so everything ends up in one log file.
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
        .map_err(|e| crate::tr!("err.open_folder", detail = e.to_string()).to_string())
}

#[derive(Serialize)]
struct ReportResult {
    url: String,
    log_truncated: bool,
}

/// Open GitHub's new-issue page with prefilled content. kind: "bug" | "feature".
#[tauri::command]
fn report_issue(app: tauri::AppHandle, kind: String, title: String, error: Option<String>) -> Result<ReportResult, String> {
    use tauri_plugin_opener::OpenerExt;
    info!("Opening GitHub issue form: {kind}");
    let link = report::issue_url(&kind, &title, error.as_deref());
    app.opener()
        .open_url(link.url.clone(), None::<&str>)
        .map_err(|e| crate::tr!("err.open_browser", detail = e.to_string()).to_string())?;
    Ok(ReportResult { url: link.url, log_truncated: link.log_truncated })
}

fn main() {
    logger::init();
    let lang = apply_language(&Config::load());
    info!("Checkpoint {} starting · {} · language={lang}", env!("CARGO_PKG_VERSION"), report::system_line());
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
            get_strings,
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
            rclone_auth_url,
            open_auth_url,
            log_frontend,
            get_log,
            open_log_folder,
            report_issue
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Checkpoint");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_strings_fall_back_to_english() {
        let en = ui_strings("en");
        let vi = ui_strings("vi");
        assert!(!en.is_empty());
        // every English key exists in the merged Vietnamese map
        for k in en.keys() {
            assert!(vi.contains_key(k), "missing key {k}");
        }
    }

    #[test]
    fn locale_files_have_same_keys() {
        let keys = |src: &str| {
            let mut m = BTreeMap::new();
            flatten("", &serde_yaml::from_str::<serde_yaml::Value>(src).unwrap(), &mut m);
            m.into_keys().filter(|k| k != "_version").collect::<Vec<_>>()
        };
        let en = keys(LOCALE_FILES[0].1);
        let vi = keys(LOCALE_FILES[1].1);
        let missing_vi: Vec<_> = en.iter().filter(|k| !vi.contains(k)).collect();
        let missing_en: Vec<_> = vi.iter().filter(|k| !en.contains(k)).collect();
        assert!(missing_vi.is_empty() && missing_en.is_empty(), "vi missing {missing_vi:?}, en missing {missing_en:?}");
    }

    /// Writes the flattened UI strings to JSON for testing the UI in a browser with a mock backend:
    /// `CHECKPOINT_DUMP=<dir> cargo test dump_ui_strings -- --ignored`
    #[test]
    #[ignore]
    fn dump_ui_strings() {
        let dir = std::env::var("CHECKPOINT_DUMP").expect("set CHECKPOINT_DUMP");
        for (lang, _) in LOCALE_FILES {
            let p = std::path::Path::new(&dir).join(format!("strings-{lang}.json"));
            std::fs::write(&p, serde_json::to_vec_pretty(&ui_strings(lang)).unwrap()).unwrap();
            println!("wrote {}", p.display());
        }
    }

    /// Every 'section.key' literal in the UI code must exist in the English "ui" strings.
    #[test]
    fn ui_code_keys_exist() {
        let en = ui_strings("en");
        let has = |k: &str| en.contains_key(k) || en.contains_key(&format!("{k}_one")) || en.contains_key(&format!("{k}_other"));
        let sections: Vec<String> = en.keys().filter_map(|k| k.split('.').next()).map(str::to_string).collect();
        let mut missing = Vec::new();
        for src in [include_str!("../../ui/app.js"), include_str!("../../ui/index.html")] {
            for quoted in src.split(['\'', '"']).skip(1).step_by(1) {
                let k = quoted.trim();
                let Some((sec, rest)) = k.split_once('.') else { continue };
                if rest.is_empty() || !sections.iter().any(|s| s == sec) {
                    continue;
                }
                if !k.chars().all(|c| c.is_ascii_lowercase() || c == '_' || c == '.') || k.ends_with('_') || k.ends_with('.') {
                    continue;
                }
                if !has(k) && !missing.contains(&k.to_string()) {
                    missing.push(k.to_string());
                }
            }
        }
        assert!(missing.is_empty(), "keys used in the UI but missing in en.yml: {missing:?}");
    }

    /// The rust-i18n locale is global; tests that change it must not run at the same time.
    static LOCALE_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn errors_shown_translated_but_logged_in_english() {
        let _l = LOCALE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        rust_i18n::set_locale("vi");
        i18n::clear();
        let err: anyhow::Error = anyhow::anyhow!("{}", crate::tr!("err.game_running", game = "Hades"));
        let shown = format!("{:#}", err.context(crate::tr!("err.snapshot")));
        rust_i18n::set_locale("en");
        assert!(shown.contains("đang chạy"), "UI text should be Vietnamese: {shown}");
        assert_eq!(
            i18n::to_en(&shown),
            "Could not take a snapshot of the local save before restoring: Hades is running. Close the game first."
        );
    }

    #[test]
    fn backend_keys_resolve() {
        let _l = LOCALE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        rust_i18n::set_locale("vi");
        assert_ne!(crate::tr!("err.no_backend"), "err.no_backend");
        rust_i18n::set_locale("en");
        assert_eq!(crate::tr!("err.game_running", game = "X"), "X is running. Close the game first.");
    }
}
