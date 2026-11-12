//! This machine's settings plus the last sync state of each game.

use anyhow::Result;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("Checkpoint")
}
pub fn cache_dir() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(|| PathBuf::from(".")).join("Checkpoint")
}
pub fn data_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("Checkpoint")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Backend {
    None,
    /// Plain folder, e.g. "G:\My Drive\Checkpoint" from Google Drive for Desktop
    Folder { path: String },
    /// rclone remote, e.g. "gdrive:Checkpoint"
    Rclone {
        remote: String,
        #[serde(default)]
        bin: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub device_id: String,
    pub device_name: String,
    pub backend: Backend,
    #[serde(default)]
    pub include_config: bool,
    #[serde(default = "default_keep")]
    pub keep_versions: usize,
    #[serde(default)]
    pub steam_root_override: Option<String>,
    /// "auto" (follow the OS), "en" or "vi"
    #[serde(default = "default_language")]
    pub language: String,
    /// Look for a new version on GitHub when the app starts
    #[serde(default = "default_true")]
    pub check_updates: bool,
    /// Also offer pre-releases (test versions) in the update check (updater.rs)
    #[serde(default)]
    pub beta_updates: bool,
    /// System notifications from the Steam launch option (notify.rs)
    #[serde(default = "default_true")]
    pub launch_notifications: bool,
    /// Launch option, when the save conflicts as the game starts: "ask" (a window with both
    /// versions), "play" (start the game, sync nothing) or "stop" (don't start the game)
    #[serde(default = "default_ask")]
    pub launch_conflict: String,
}

fn default_ask() -> String {
    "ask".into()
}

fn default_true() -> bool {
    true
}

fn default_keep() -> usize {
    10
}

fn default_language() -> String {
    "auto".into()
}

pub const LANGUAGES: &[&str] = &["en", "vi"];

impl Config {
    /// The UI language actually in use: the configured one, or the OS language when set to "auto".
    pub fn effective_language(&self) -> String {
        if LANGUAGES.contains(&self.language.as_str()) {
            return self.language.clone();
        }
        let sys = sys_locale::get_locale().unwrap_or_default().to_lowercase();
        LANGUAGES.iter().find(|l| sys.starts_with(*l)).unwrap_or(&"en").to_string()
    }
}

fn host_name() -> String {
    sysinfo::System::host_name()
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .unwrap_or_else(|| "this-pc".into())
}

impl Config {
    fn fresh() -> Self {
        let id: u64 = rand::thread_rng().gen();
        Config {
            device_id: format!("{:016x}", id),
            device_name: host_name(),
            backend: Backend::None,
            include_config: false,
            keep_versions: 10,
            steam_root_override: None,
            language: default_language(),
            check_updates: true,
            beta_updates: false,
            launch_notifications: true,
            launch_conflict: default_ask(),
        }
    }

    pub fn load() -> Self {
        let p = config_dir().join("config.json");
        if let Ok(data) = fs::read(&p) {
            if let Ok(c) = serde_json::from_slice::<Config>(&data) {
                return c;
            }
        }
        let c = Config::fresh();
        let _ = c.save();
        c
    }

    pub fn save(&self) -> Result<()> {
        fs::create_dir_all(config_dir())?;
        fs::write(config_dir().join("config.json"), serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }
}

/// Last sync on this machine: the local hash and the cloud hash at that moment.
/// Used to tell which side changed since the previous sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncBase {
    pub local: String,
    pub cloud: String,
    pub at_utc: i64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub games: HashMap<String, SyncBase>,
}

impl State {
    pub fn load() -> Self {
        fs::read(config_dir().join("state.json"))
            .ok()
            .and_then(|d| serde_json::from_slice(&d).ok())
            .unwrap_or_default()
    }
    pub fn save(&self) -> Result<()> {
        fs::create_dir_all(config_dir())?;
        fs::write(config_dir().join("state.json"), serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }
}
