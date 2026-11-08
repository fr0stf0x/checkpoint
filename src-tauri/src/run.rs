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
//! game is started regardless: the wrapper never blocks playing. Everything is logged with the
//! game's appid, so a per-game bug report includes it.

use crate::config::{Backend, Config};
use crate::sync;
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
    if let (true, Some(id)) = (sync_on, a.appid) {
        before(&cfg, id);
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
        after(&cfg, id);
    }
    crate::rcd::shutdown();
    code
}

fn before(cfg: &Config, appid: u32) {
    let st = match sync::game_status(cfg, appid) {
        Ok(s) => s,
        Err(e) => return warn!("Launch wrapper appid={appid}: status check failed, starting without restore: {}", crate::i18n::to_en(&format!("{e:#}"))),
    };
    if st.status != "cloud" {
        return info!("Launch wrapper appid={appid}: {} is {}, nothing to restore", st.name, st.status);
    }
    match sync::restore(cfg, appid, None, false, st.remap) {
        Ok(r) => info!("Launch wrapper appid={appid}: restore {}", r.outcome),
        Err(e) => warn!("Launch wrapper appid={appid}: restore skipped: {}", crate::i18n::to_en(&format!("{e:#}"))),
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

fn after(cfg: &Config, appid: u32) {
    let st = match sync::game_status(cfg, appid) {
        Ok(s) => s,
        Err(e) => return warn!("Launch wrapper appid={appid}: status check failed, no backup: {}", crate::i18n::to_en(&format!("{e:#}"))),
    };
    if !matches!(st.status, "local" | "new") {
        return info!("Launch wrapper appid={appid}: {} is {} after playing, no automatic backup", st.name, st.status);
    }
    match sync::backup(cfg, appid, false) {
        Ok(r) => info!("Launch wrapper appid={appid}: backup {}", r.outcome),
        Err(e) => warn!("Launch wrapper appid={appid}: backup skipped: {}", crate::i18n::to_en(&format!("{e:#}"))),
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
