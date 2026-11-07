//! Cấu hình máy này + trạng thái đồng bộ lần cuối của từng game.

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
    /// Thư mục thường: ví dụ "G:\My Drive\Checkpoint" của Google Drive for Desktop
    Folder { path: String },
    /// rclone remote, ví dụ "gdrive:Checkpoint"
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
}

fn default_keep() -> usize {
    10
}

fn host_name() -> String {
    sysinfo::System::host_name()
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .unwrap_or_else(|| "may-nay".into())
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

/// Lần đồng bộ cuối trên máy này: hash local lúc đó và hash bản cloud lúc đó.
/// Dùng để biết bên nào đã thay đổi kể từ lần đồng bộ trước.
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
