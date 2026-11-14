//! The question the launch wrapper asks when a game starts while its save conflicts (both this PC
//! and the cloud changed since the last sync, or this PC never synced and differs): use the cloud
//! save, keep this PC's save, or don't start the game.
//!
//! The wrapper writes both sides to a JSON file and runs this executable again as
//! `checkpoint prompt <file>`, which opens a small Checkpoint window (`window.__CHECKPOINT_PROMPT__`
//! in the UI). The window writes the answer into the same file and quits. The window process
//! never touches the cloud: everything it shows comes from the file. In Gaming Mode the window
//! shows where the game would, since Steam sees it as part of the game.

use crate::sync::ConflictInfo;
use anyhow::{Context, Result};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Choice {
    /// Restore the cloud save, then play
    Cloud,
    /// Play with this PC's save; back it up over the cloud afterwards
    Local,
    /// Don't start the game
    Cancel,
}

#[derive(Serialize, Deserialize)]
struct PromptFile {
    info: ConflictInfo,
    answer: Option<Choice>,
}

// ---------- launch wrapper side ----------

/// Ask in a window. None when the window could not be shown (no display, crash): the caller then
/// starts the game as before. Closing the window without an answer counts as Cancel.
pub fn ask(info: &ConflictInfo) -> Option<Choice> {
    let path = crate::config::cache_dir().join("tmp").join(format!("prompt-{}.json", std::process::id()));
    let r = run_prompt(&path, info);
    let _ = fs::remove_file(&path);
    match r {
        Ok(c) => {
            info!("Launch wrapper appid={}: answer {c:?}", info.appid);
            Some(c)
        }
        Err(e) => {
            warn!("Launch wrapper appid={}: could not ask about the conflict: {e:#}", info.appid);
            None
        }
    }
}

fn run_prompt(path: &Path, info: &ConflictInfo) -> Result<Choice> {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    fs::write(path, serde_json::to_vec(&PromptFile { info: info.clone(), answer: None })?)?;
    let exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("prompt").arg(path);
    without_steam_libs(&mut cmd);
    let status = cmd.status().context("start the prompt window")?;
    let file: PromptFile = serde_json::from_slice(&fs::read(path)?)?;
    match file.answer {
        Some(c) => Ok(c),
        None if status.success() => Ok(Choice::Cancel),
        None => anyhow::bail!("prompt window exited with {status}"),
    }
}

/// On Linux, Steam starts the launch options with its overlay in `LD_PRELOAD` and its runtime's
/// libraries in `LD_LIBRARY_PATH`. The game needs them (the wrapper passes its environment on
/// unchanged), but they can break the prompt window's own libraries (WebKitGTK in the AppImage):
/// leave out Steam's entries for the window only.
fn without_steam_libs(cmd: &mut std::process::Command) {
    if !cfg!(target_os = "linux") {
        return;
    }
    let steam = |p: &str| p.contains("gameoverlayrenderer") || p.contains("steam-runtime") || p.contains("ubuntu12_");
    for (var, sep) in [("LD_PRELOAD", [' ', ':']), ("LD_LIBRARY_PATH", [':', ':'])] {
        let Ok(v) = std::env::var(var) else { continue };
        let kept: Vec<&str> = v.split(&sep[..]).filter(|p| !p.is_empty() && !steam(p)).collect();
        if kept.is_empty() {
            cmd.env_remove(var);
        } else {
            cmd.env(var, kept.join(&sep[1].to_string()));
        }
    }
}

// ---------- prompt window side ----------

static FILE: OnceLock<PathBuf> = OnceLock::new();

/// `checkpoint prompt <file>`: remember the file; the app then opens the small prompt window.
pub fn set_file(path: PathBuf) {
    let _ = FILE.set(path);
}

pub fn active() -> bool {
    FILE.get().is_some()
}

pub fn read_info() -> Result<ConflictInfo> {
    let path = FILE.get().context("not in prompt mode")?;
    let file: PromptFile = serde_json::from_slice(&fs::read(path)?)?;
    Ok(file.info)
}

pub fn write_answer(choice: Choice) -> Result<()> {
    let path = FILE.get().context("not in prompt mode")?;
    let mut file: PromptFile = serde_json::from_slice(&fs::read(path)?)?;
    file.answer = Some(choice);
    fs::write(path, serde_json::to_vec(&file)?)?;
    Ok(())
}
