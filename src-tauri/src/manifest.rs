//! Ludusavi's save-location manifest (community data from PCGamingWiki).
//! Downloads the YAML once, keeps only games with a Steam AppID and caches it as a small JSON.

use anyhow::{Context, Result};
use crate::tr;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::PathBuf;

pub const MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/mtkennerly/ludusavi-manifest/master/data/manifest.yaml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct When {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRule {
    pub path: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub when: Vec<When>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub files: Vec<FileRule>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Index {
    pub updated_utc: i64,
    pub games: HashMap<u32, Entry>,
}

#[derive(Deserialize)]
struct RawFile {
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    when: Vec<When>,
}

#[derive(Deserialize)]
struct RawSteam {
    #[serde(default)]
    id: Option<u32>,
}

#[derive(Deserialize)]
struct RawGame {
    #[serde(default)]
    files: HashMap<String, Option<RawFile>>,
    #[serde(default)]
    steam: Option<RawSteam>,
}

fn index_path() -> PathBuf {
    crate::config::cache_dir().join("manifest-index.json")
}

pub fn load() -> Option<Index> {
    let data = fs::read(index_path()).ok()?;
    serde_json::from_slice(&data).ok()
}

pub fn download_and_build() -> Result<Index> {
    let resp = ureq::get(MANIFEST_URL)
        .timeout(std::time::Duration::from_secs(180))
        .call()
        .context(tr!("err.manifest_download").to_string())?;
    let mut buf = Vec::new();
    resp.into_reader().read_to_end(&mut buf)?;
    let raw: HashMap<String, RawGame> =
        serde_yaml::from_slice(&buf).context(tr!("err.manifest_parse").to_string())?;

    let mut games = HashMap::new();
    for (name, g) in raw {
        let Some(id) = g.steam.and_then(|s| s.id) else { continue };
        let files: Vec<FileRule> = g
            .files
            .into_iter()
            .map(|(path, f)| {
                let f = f.unwrap_or(RawFile { tags: vec![], when: vec![] });
                FileRule { path, tags: f.tags, when: f.when }
            })
            .collect();
        if files.is_empty() {
            continue;
        }
        games.insert(id, Entry { name, files });
    }
    let idx = Index { updated_utc: chrono::Utc::now().timestamp_millis(), games };
    let p = index_path();
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&p, serde_json::to_vec(&idx)?)?;
    Ok(idx)
}
