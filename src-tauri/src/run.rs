//! Launch wrapper for Steam's launch options:
//!
//! ```text
//! "<path to Checkpoint>" run --appid <appid> -- %command%
//! ```
//!
//! Steam replaces `%command%` with the game's own command line. Before starting it, Checkpoint
//! restores the cloud save if the cloud is newer; after the game (and anything it started from
//! its install folder) has quit, it backs up if this PC's save is newer. Anything that needs a
//! decision (conflict, never synced, cloud newer after playing) is left for the app, and the
//! game is started (conflicts at the start: see below). Everything is logged with the
//! game's appid, so a per-game bug report includes it. What happened is also shown as a system
//! notification (notify.rs; can be turned off in Settings): a restore or backup that ran, one
//! that failed, or a save left for the app to decide.
//!
//! A conflict when the game starts (both sides changed, or this PC never synced and differs)
//! follows the `launch_conflict` setting: "ask" opens a small window (prompt.rs) to use the cloud
//! save, keep this PC's save (backed up over the cloud after playing) or not start the game;
//! "play" starts it without syncing; "stop" doesn't start it. If the window cannot open, or the
//! cloud cannot be reached, the game starts.

use crate::config::{Backend, Config};
use crate::prompt::{self, Choice};
use crate::{notify, sync};
use rust_i18n::t;
use log::{error, info, warn};
use std::process::Command;
use std::time::{Duration, Instant};

struct Args {
    appid: Option<u32>,
    command: Vec<String>,
}

/// `[--appid N] [--] command…`. Without `--appid`, Steam's `SteamAppId` variable is used.
fn parse(args: &[String]) -> Args {
    let mut appid = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--appid" if i + 1 < args.len() => {
                appid = args[i + 1].parse().ok();
                i += 2;
            }
            "--" => {
                i += 1;
                break;
            }
            _ => break,
        }
    }
    let appid = appid.or_else(|| ["SteamAppId", "SteamGameId"].iter().find_map(|k| std::env::var(k).ok()?.parse().ok()));
    Args { appid: appid.filter(|a| *a != 0), command: args[i..].to_vec() }
}

/// Entry point for `checkpoint run …`. Returns the game's exit code.
pub fn main(args: &[String]) -> i32 {
    let a = parse(args);
    let Some(program) = a.command.first() else {
        error!("Launch wrapper: no game command given (use: run --appid <appid> -- %command%)");
        return 2;
    };
    let cfg = Config::load();
    let sync_on = a.appid.is_some() && !matches!(cfg.backend, Backend::None);
    match a.appid {
        Some(id) if sync_on => info!("Launch wrapper appid={id}: starting"),
        Some(id) => info!("Launch wrapper appid={id}: no backup location set, only starting the game"),
        None => warn!("Launch wrapper: no appid (add --appid), only starting the game"),
    }
    let mut keep_local = false;
    if let (true, Some(id)) = (sync_on, a.appid) {
        match before(&cfg, id) {
            Start::Play { keep_local: k } => keep_local = k,
            Start::Stop => {
                info!("Launch wrapper appid={id}: game not started");
                crate::rcd::shutdown();
                return 0;
            }
        }
    }

    let code = match Command::new(program).args(&a.command[1..]).status() {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            error!("Launch wrapper{}: could not start the game ({program}): {e}", a.appid.map(|i| format!(" appid={i}")).unwrap_or_default());
            crate::rcd::shutdown();
            return 1;
        }
    };

    if let (true, Some(id)) = (sync_on, a.appid) {
        info!("Launch wrapper appid={id}: game command exited with {code}");
        wait_until_closed(&cfg, id);
        after(&cfg, id, keep_local);
    }
    crate::rcd::shutdown();
    code
}

/// The error as shown to the user, and in English for the log.
fn error_text(e: &anyhow::Error) -> (String, String) {
    let shown = format!("{e:#}");
    let en = crate::i18n::to_en(&shown);
    (shown, en)
}

/// The save changed on both sides (or the cloud got newer while playing): nothing is synced
/// automatically, the user picks in the app.
fn notify_decide(cfg: &Config, game: &str) {
    notify::show(cfg, &t!("msg.notify_decide_title", game = game), &t!("msg.notify_decide_body"));
}

/// What to do after the check before the game.
enum Start {
    /// Start the game; `keep_local`: the user chose this PC's save over a conflicting cloud
    /// version, so back it up afterwards even though the cloud changed too.
    Play { keep_local: bool },
    /// The user (or the "stop" setting) chose not to start the game.
    Stop,
}

const PLAY: Start = Start::Play { keep_local: false };

fn before(cfg: &Config, appid: u32) -> Start {
    let st = match sync::game_status(cfg, appid) {
        Ok(s) => s,
        Err(e) => {
            // Offline or no access to the cloud: nothing is known, so the game starts
            let (shown, en) = error_text(&e);
            warn!("Launch wrapper appid={appid}: status check failed, starting without restore: {en}");
            notify::show(cfg, &t!("msg.notify_restore_failed"), &shown);
            return PLAY;
        }
    };
    if matches!(st.status, "conflict" | "diverged") {
        info!("Launch wrapper appid={appid}: {} is {}, on conflict: {}", st.name, st.status, cfg.launch_conflict);
        return match cfg.launch_conflict.as_str() {
            "play" => {
                notify_decide(cfg, &st.name);
                PLAY
            }
            "stop" => {
                notify::show_always(&t!("msg.notify_stopped_title", game = st.name), &t!("msg.notify_decide_body"));
                Start::Stop
            }
            _ => match prompt::ask(&st.info) {
                Some(Choice::Cloud) => {
                    restore(cfg, appid, &st, true);
                    PLAY
                }
                Some(Choice::Local) => Start::Play { keep_local: true },
                Some(Choice::Cancel) => Start::Stop,
                // The window could not open: start the game as before
                None => {
                    notify_decide(cfg, &st.name);
                    PLAY
                }
            },
        };
    }
    if st.status == "cloud" {
        restore(cfg, appid, &st, false);
    } else {
        info!("Launch wrapper appid={appid}: {} is {}, nothing to restore", st.name, st.status);
    }
    PLAY
}

/// `force`: the user chose the cloud save over a conflicting local one.
fn restore(cfg: &Config, appid: u32, st: &sync::GameStatus, force: bool) {
    match sync::restore(cfg, appid, None, force, st.remap.clone(), &sync::no_progress) {
        Ok(r) => {
            info!("Launch wrapper appid={appid}: restore {}", r.outcome);
            if r.outcome == "restored" {
                notify::show(cfg, "Checkpoint", &r.message);
            }
        }
        Err(e) => {
            let (shown, en) = error_text(&e);
            warn!("Launch wrapper appid={appid}: restore skipped: {en}");
            if shown.starts_with("NEEDS_CONFIRM") {
                notify_decide(cfg, &st.name);
            } else {
                notify::show(cfg, &t!("msg.notify_restore_failed"), &shown);
            }
        }
    }
}

/// Some games exit their launcher right away and keep running in another process: wait until
/// nothing runs from the install folder any more (checked every 5 s, at most 24 h).
fn wait_until_closed(cfg: &Config, appid: u32) {
    let Ok(base) = sync::game_base(cfg, appid) else { return };
    let start = Instant::now();
    std::thread::sleep(Duration::from_secs(2));
    let mut logged = false;
    while sync::Procs::snapshot().running(&base) && start.elapsed() < Duration::from_secs(24 * 3600) {
        if !logged {
            info!("Launch wrapper appid={appid}: game still running after its launcher quit, waiting");
            logged = true;
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}

fn after(cfg: &Config, appid: u32, keep_local: bool) {
    let st = match sync::game_status(cfg, appid) {
        Ok(s) => s,
        Err(e) => {
            let (shown, en) = error_text(&e);
            warn!("Launch wrapper appid={appid}: status check failed, no backup: {en}");
            return notify::show(cfg, &t!("msg.notify_backup_failed"), &shown);
        }
    };
    // Chosen before playing: this PC's save wins over the cloud version (which stays in History)
    let force = keep_local && matches!(st.status, "local" | "new" | "conflict" | "diverged");
    if !force && !matches!(st.status, "local" | "new") {
        info!("Launch wrapper appid={appid}: {} is {} after playing, no automatic backup", st.name, st.status);
        if matches!(st.status, "cloud" | "conflict" | "diverged") {
            notify_decide(cfg, &st.name);
        }
        return;
    }
    match sync::backup(cfg, appid, force, &sync::no_progress) {
        Ok(r) => {
            info!("Launch wrapper appid={appid}: backup {}", r.outcome);
            if r.outcome == "uploaded" {
                notify::show(cfg, "Checkpoint", &r.message);
            }
        }
        Err(e) => {
            let (shown, en) = error_text(&e);
            warn!("Launch wrapper appid={appid}: backup skipped: {en}");
            if shown.starts_with("NEEDS_CONFIRM") {
                notify_decide(cfg, &st.name);
            } else {
                notify::show(cfg, &t!("msg.notify_backup_failed"), &shown);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn parses_launch_options() {
        let a = parse(&v(&["--appid", "646570", "--", "C:\\Games\\SlayTheSpire.exe", "-x"]));
        assert_eq!(a.appid, Some(646570));
        assert_eq!(a.command, v(&["C:\\Games\\SlayTheSpire.exe", "-x"]));
        // Steam's command may itself contain "--" (Proton's reaper): only the first one splits
        let a = parse(&v(&["--appid", "1", "--", "reaper", "SteamLaunch", "AppId=1", "--", "proton", "run"]));
        assert_eq!(a.command, v(&["reaper", "SteamLaunch", "AppId=1", "--", "proton", "run"]));
        let a = parse(&v(&["--", "game"]));
        assert_eq!(a.command, v(&["game"]));
    }
}
