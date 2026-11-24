//! Writes the launch wrapper (run.rs) into a game's Steam launch options, so the user does not
//! have to paste it into every game, on every PC.
//!
//! Launch options live in `userdata/<accountid>/config/localconfig.vdf` (text VDF):
//! `UserLocalConfigStore → Software → Valve → Steam → apps → <appid> → "LaunchOptions"`.
//! Steam keeps the file in memory and rewrites it when it exits, so Steam must be closed while we
//! write. Only the bytes of that one value change (or one line / one small block is inserted): the
//! rest of the file stays byte for byte. A copy of the old file is kept next to it.
//!
//! Existing options are kept: the wrapper goes after leading `VAR=value` assignments (shell syntax
//! must stay first) and before any other wrapper, so `~/.lsfg %command%` (lsfg-vk frame
//! generation on Bazzite) becomes `"<checkpoint>" run --appid N -- ~/.lsfg %command%`. Other
//! wrappers then only apply to the game, not to Checkpoint's conflict window. Options without
//! `%command%` are game arguments: `-dx11` becomes `"<checkpoint>" run --appid N -- %command% -dx11`.
//! What the game had before is stored in `<data>/launch_options.json`, so turning it off restores
//! the options exactly.

use crate::tr;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

// ---------- launch option text ----------

/// One shell word of a launch option, with its byte range in the text.
struct Word {
    range: Range<usize>,
    /// Without quotes and escapes
    value: String,
    /// Starts with a quote or backslash (then it cannot be a `VAR=value` assignment)
    quoted: bool,
}

/// Splits like `sh` does for plain words, '…', "…" and backslash escapes (enough to find words;
/// no expansions).
fn words(s: &str) -> Vec<Word> {
    let b: Vec<(usize, char)> = s.char_indices().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i].1.is_whitespace() {
            i += 1;
            continue;
        }
        let start = b[i].0;
        let quoted = matches!(b[i].1, '"' | '\'' | '\\');
        let mut value = String::new();
        let mut quote: Option<char> = None;
        while i < b.len() {
            let c = b[i].1;
            match quote {
                None if c.is_whitespace() => break,
                None if c == '"' || c == '\'' => quote = Some(c),
                None if c == '\\' && i + 1 < b.len() => {
                    i += 1;
                    value.push(b[i].1);
                }
                Some(q) if c == q => quote = None,
                Some('"') if c == '\\' && i + 1 < b.len() && matches!(b[i + 1].1, '"' | '\\' | '$' | '`') => {
                    i += 1;
                    value.push(b[i].1);
                }
                _ => value.push(c),
            }
            i += 1;
        }
        let end = b.get(i).map(|x| x.0).unwrap_or(s.len());
        out.push(Word { range: start..end, value, quoted });
    }
    out
}

fn is_assignment(w: &Word) -> bool {
    if w.quoted {
        return false;
    }
    let Some((name, _)) = w.value.split_once('=') else { return false };
    let mut c = name.chars();
    c.next().is_some_and(|f| f.is_ascii_alphabetic() || f == '_') && c.all(|x| x.is_ascii_alphanumeric() || x == '_')
}

fn file_name(p: &str) -> String {
    Path::new(&p.replace('\\', "/")).file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// The wrapper inside a launch option: its byte range (from the program through `--` and the
/// spaces after it), the Checkpoint path and the appid it names.
struct Found {
    range: Range<usize>,
    path: String,
    appid: Option<u32>,
}

fn find(opts: &str) -> Option<Found> {
    let w = words(opts);
    let i = (0..w.len()).find(|&i| file_name(&w[i].value).starts_with("checkpoint") && w.get(i + 1).is_some_and(|x| x.value == "run"))?;
    let dashes = (i + 2..w.len()).find(|&j| w[j].value == "--")?;
    let appid = (i + 2..dashes).find(|&j| w[j].value == "--appid").and_then(|j| w.get(j + 1)).and_then(|x| x.value.parse().ok());
    // Up to the next word, so removing the range leaves the rest as it was
    let end = w.get(dashes + 1).map(|x| x.range.start).unwrap_or(opts.len());
    Some(Found { range: w[i].range.start..end, path: w[i].value.clone(), appid })
}

/// `"<exe>" run --appid N -- `
fn prefix(exe: &str, appid: u32) -> String {
    format!("\"{exe}\" run --appid {appid} -- ")
}

/// The launch option with the wrapper for `exe`, keeping what the game already has. A wrapper
/// already in it (also one pointing to an old Checkpoint path) is replaced.
pub fn merge(opts: &str, exe: &str, appid: u32) -> String {
    let opts = unmerge(opts);
    let trimmed = opts.trim();
    if trimmed.is_empty() || !trimmed.contains("%command%") {
        let rest = if trimmed.is_empty() { String::new() } else { format!(" {trimmed}") };
        return format!("{}%command%{rest}", prefix(exe, appid));
    }
    let w = words(&opts);
    let at = w.iter().find(|w| !is_assignment(w)).map(|w| w.range.start).unwrap_or(opts.len());
    format!("{}{}{}", &opts[..at], prefix(exe, appid), &opts[at..])
}

/// The launch option without the wrapper (unchanged when it has none).
pub fn unmerge(opts: &str) -> String {
    let Some(f) = find(opts) else { return opts.to_string() };
    let out = format!("{}{}", &opts[..f.range.start], &opts[f.range.end..]);
    // Only `%command%` left: that is what an empty launch option does
    if out.trim() == "%command%" {
        String::new()
    } else {
        out
    }
}

fn same_path(a: &str, b: &str) -> bool {
    let n = |s: &str| {
        let s = s.replace('\\', "/");
        if cfg!(windows) { s.to_lowercase() } else { s }
    };
    n(a) == n(b)
}

/// "off": no wrapper; "on": the wrapper for this Checkpoint and this game; "old": a wrapper with
/// another Checkpoint path (moved AppImage) or another appid, which needs repairing.
fn state_of(opts: &str, exe: &str, appid: u32) -> &'static str {
    match find(opts) {
        None => "off",
        Some(f) if same_path(&f.path, exe) && f.appid == Some(appid) => "on",
        Some(_) => "old",
    }
}

// ---------- localconfig.vdf, edited in place ----------

enum Val {
    /// Byte range of the quoted value, and the value
    Str(Range<usize>, String),
    /// Byte position just after `{`, and the children
    Obj(usize, Vec<(String, Val)>),
}

struct Parser<'a> {
    s: &'a str,
    b: &'a [u8],
    i: usize,
}

enum Tok {
    S(Range<usize>, String),
    Open(usize),
    Close,
    End,
}

impl Parser<'_> {
    fn tok(&mut self) -> Result<Tok> {
        loop {
            while self.i < self.b.len() && (self.b[self.i].is_ascii_whitespace()) {
                self.i += 1;
            }
            if self.s[self.i..].starts_with('\u{feff}') {
                self.i += 3;
                continue;
            }
            if self.s[self.i..].starts_with("//") {
                while self.i < self.b.len() && self.b[self.i] != b'\n' {
                    self.i += 1;
                }
                continue;
            }
            break;
        }
        let Some(&c) = self.b.get(self.i) else { return Ok(Tok::End) };
        let start = self.i;
        match c {
            b'{' => {
                self.i += 1;
                Ok(Tok::Open(self.i))
            }
            b'}' => {
                self.i += 1;
                Ok(Tok::Close)
            }
            b'"' => {
                self.i += 1;
                let mut v = String::new();
                loop {
                    let ch = self.s[self.i..].chars().next().context("unterminated string")?;
                    self.i += ch.len_utf8();
                    match ch {
                        '"' => break,
                        '\\' => {
                            let e = self.s[self.i..].chars().next().context("unterminated string")?;
                            self.i += e.len_utf8();
                            v.push(match e {
                                'n' => '\n',
                                't' => '\t',
                                o => o,
                            });
                        }
                        o => v.push(o),
                    }
                }
                Ok(Tok::S(start..self.i, v))
            }
            _ => {
                while self.i < self.b.len() && !self.b[self.i].is_ascii_whitespace() && !matches!(self.b[self.i], b'{' | b'}' | b'"') {
                    self.i += 1;
                }
                Ok(Tok::S(start..self.i, self.s[start..self.i].to_string()))
            }
        }
    }

    /// Children until `}` (or the end of the file at the top level)
    fn obj(&mut self, top: bool) -> Result<Vec<(String, Val)>> {
        let mut out = Vec::new();
        loop {
            match self.tok()? {
                Tok::End if top => return Ok(out),
                Tok::Close if !top => return Ok(out),
                Tok::S(_, key) => match self.tok()? {
                    Tok::Open(at) => out.push((key, Val::Obj(at, self.obj(false)?))),
                    Tok::S(r, v) => {
                        // Conditionals such as [$WIN32] after a value
                        let save = self.i;
                        if let Tok::S(_, c) = self.tok()? {
                            if !c.starts_with('[') {
                                self.i = save;
                            }
                        } else {
                            self.i = save;
                        }
                        out.push((key, Val::Str(r, v)))
                    }
                    _ => bail!("key without value"),
                },
                _ => bail!("unexpected token"),
            }
        }
    }
}

fn parse(s: &str) -> Result<Vec<(String, Val)>> {
    Parser { s, b: s.as_bytes(), i: 0 }.obj(true)
}

fn child<'a>(v: &'a [(String, Val)], key: &str) -> Option<&'a Val> {
    v.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v)
}

/// The `apps` block: position after its `{` and its children.
fn apps(root: &[(String, Val)]) -> Option<(usize, &[(String, Val)])> {
    let mut cur = root;
    for key in ["UserLocalConfigStore", "Software", "Valve", "Steam"] {
        match child(cur, key)? {
            Val::Obj(_, c) => cur = c,
            Val::Str(..) => return None,
        }
    }
    match child(cur, "apps")? {
        Val::Obj(at, c) => Some((*at, c)),
        Val::Str(..) => None,
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Every game's launch options in the file.
fn read_all(text: &str) -> Result<HashMap<u32, String>> {
    let root = parse(text)?;
    let Some((_, list)) = apps(&root) else { return Ok(HashMap::new()) };
    Ok(list
        .iter()
        .filter_map(|(k, v)| match v {
            Val::Obj(_, c) => match child(c, "LaunchOptions") {
                Some(Val::Str(_, s)) => Some((k.parse().ok()?, s.clone())),
                _ => Some((k.parse().ok()?, String::new())),
            },
            Val::Str(..) => None,
        })
        .collect())
}

/// The whitespace at the start of the line holding byte `at`.
fn indent_at(text: &str, at: usize) -> &str {
    let line = text[..at].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let n = text[line..].bytes().take_while(|c| *c == b' ' || *c == b'\t').count();
    &text[line..line + n]
}

/// The file with `appid`'s launch options set to `value`; everything else stays as it was.
fn set_in(text: &str, appid: u32, value: &str) -> Result<String> {
    let root = parse(text)?;
    let (apps_at, list) = apps(&root).context("no apps block")?;
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let quoted = format!("\"{}\"", escape(value));
    let (at, insert, replace_end) = match child(list, &appid.to_string()) {
        Some(Val::Obj(at, c)) => match child(c, "LaunchOptions") {
            Some(Val::Str(r, _)) => (r.start, quoted, r.end),
            Some(Val::Obj(..)) => bail!("LaunchOptions is a block"),
            None => {
                let ind = format!("{}\t", indent_at(text, *at));
                (*at, format!("{nl}{ind}\"LaunchOptions\"\t\t{quoted}"), *at)
            }
        },
        Some(Val::Str(..)) => bail!("app entry is a value"),
        None => {
            let ind = format!("{}\t", indent_at(text, apps_at));
            (apps_at, format!("{nl}{ind}\"{appid}\"{nl}{ind}{{{nl}{ind}\t\"LaunchOptions\"\t\t{quoted}{nl}{ind}}}"), apps_at)
        }
    };
    Ok(format!("{}{}{}", &text[..at], insert, &text[replace_end..]))
}

// ---------- what Checkpoint wrote ----------

/// What each game had before Checkpoint changed it, per Steam account.
#[derive(Default, Serialize, Deserialize)]
struct Stored {
    #[serde(default)]
    accounts: BTreeMap<String, BTreeMap<String, Entry>>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    original: String,
    written: String,
}

fn stored_path() -> PathBuf {
    crate::config::data_dir().join("launch_options.json")
}

fn load_stored() -> Stored {
    fs::read(stored_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_stored(s: &Stored) -> Result<()> {
    let p = stored_path();
    if let Some(d) = p.parent() {
        fs::create_dir_all(d)?;
    }
    fs::write(&p, serde_json::to_vec_pretty(s)?).with_context(|| tr!("err.write_file", path = p.display()).to_string())
}

// ---------- commands ----------

#[derive(Debug, Serialize)]
pub struct GameLaunch {
    /// The launch options in Steam now
    pub current: String,
    /// "off", "on" or "old" (see `state_of`)
    pub state: &'static str,
    /// The launch options with the wrapper (to copy by hand)
    pub merged: String,
}

#[derive(Debug, Serialize)]
pub struct LaunchInfo {
    /// The Steam account whose launch options these are (signed in most recently on this PC)
    pub account: String,
    pub steam_running: bool,
    pub games: HashMap<u32, GameLaunch>,
}

fn account(steam_root: &Path) -> Result<(crate::steam::Account, PathBuf)> {
    let accts = crate::steam::accounts(steam_root);
    let a = accts.into_iter().find(|a| a.most_recent).context(tr!("err.no_steam_account").to_string())?;
    let p = steam_root.join("userdata").join(a.account_id.to_string()).join("config").join("localconfig.vdf");
    Ok((a, p))
}

fn display_name(a: &crate::steam::Account) -> String {
    if a.persona_name.is_empty() { a.account_name.clone() } else { a.persona_name.clone() }
}

fn exe() -> Result<String> {
    Ok(crate::shortcuts::launcher()?.to_string_lossy().to_string())
}

/// Launch options of `appids` (and of every other game that has the wrapper).
pub fn states(steam_root: &Path, appids: &[u32]) -> Result<LaunchInfo> {
    let (a, path) = account(steam_root)?;
    let exe = exe()?;
    let all = match fs::read_to_string(&path) {
        Ok(t) => read_all(&t).unwrap_or_else(|e| {
            log::warn!("Launch options: localconfig.vdf not readable: {e:#}");
            HashMap::new()
        }),
        Err(_) => HashMap::new(),
    };
    let mut games = HashMap::new();
    let with_wrapper = all.iter().filter(|(_, o)| find(o).is_some()).map(|(id, _)| *id);
    for id in appids.iter().copied().chain(with_wrapper) {
        let current = all.get(&id).cloned().unwrap_or_default();
        games.insert(id, GameLaunch { state: state_of(&current, &exe, id), merged: merge(&current, &exe, id), current });
    }
    Ok(LaunchInfo { account: display_name(&a), steam_running: crate::shortcuts::steam_running(), games })
}

#[derive(Debug, Serialize)]
pub struct SetResult {
    pub changed: usize,
    pub account: String,
}

/// Turns the wrapper on or off for `appids`. "On" also repairs a wrapper with an old path.
pub fn set(steam_root: &Path, appids: &[u32], on: bool) -> Result<SetResult> {
    if crate::shortcuts::steam_running() {
        bail!("{}", tr!("err.steam_running"));
    }
    let (a, path) = account(steam_root)?;
    let exe = exe()?;
    let name = display_name(&a);
    let orig_text = fs::read_to_string(&path).with_context(|| tr!("err.launch_no_config", account = name).to_string())?;
    let all = read_all(&orig_text).map_err(|e| anyhow::anyhow!("{}: {e:#}", tr!("err.launch_config_format")))?;
    let mut stored = load_stored();
    let mine = stored.accounts.entry(a.account_id.to_string()).or_default();
    let mut text = orig_text.clone();
    let mut changed = Vec::new();
    for &id in appids {
        let current = all.get(&id).cloned().unwrap_or_default();
        let new = if on {
            merge(&current, &exe, id)
        } else {
            match mine.get(&id.to_string()) {
                // Unchanged since Checkpoint wrote it: put back exactly what was there
                Some(e) if e.written == current => e.original.clone(),
                _ => unmerge(&current),
            }
        };
        if new == current {
            continue;
        }
        text = set_in(&text, id, &new).map_err(|e| anyhow::anyhow!("{}: {e:#}", tr!("err.launch_config_format")))?;
        changed.push((id, current, new));
    }
    if changed.is_empty() {
        return Ok(SetResult { changed: 0, account: name });
    }
    // Read back: every game must have what we meant, and no other game may change
    let check = read_all(&text).ok();
    let ok = check.as_ref().is_some_and(|c| {
        changed.iter().all(|(id, _, new)| c.get(id).map(String::as_str).unwrap_or("") == new)
            && all.iter().filter(|(id, _)| !changed.iter().any(|(c, ..)| c == *id)).all(|(id, o)| c.get(id) == Some(o))
    });
    if !ok {
        bail!("{}", tr!("err.launch_config_format"));
    }
    let dir = path.parent().context("no config folder")?;
    let bak = dir.join("localconfig.vdf.checkpoint-bak");
    fs::write(&bak, &orig_text).with_context(|| tr!("err.write_file", path = bak.display()).to_string())?;
    let tmp = dir.join("localconfig.vdf.checkpoint-tmp");
    fs::write(&tmp, &text).with_context(|| tr!("err.write_file", path = tmp.display()).to_string())?;
    fs::rename(&tmp, &path).with_context(|| tr!("err.write_file", path = path.display()).to_string())?;
    for (id, current, new) in &changed {
        let key = id.to_string();
        if on {
            // Keep the first original (a repair or a second "on" must not store the wrapper itself)
            let original = mine.get(&key).filter(|e| e.written == *current).map(|e| e.original.clone()).unwrap_or_else(|| unmerge(current));
            mine.insert(key, Entry { original, written: new.clone() });
        } else {
            mine.remove(&key);
        }
        log::info!("Launch options appid={id}: automatic sync {}", if on { "on" } else { "off" });
    }
    save_stored(&stored)?;
    Ok(SetResult { changed: changed.len(), account: name })
}

/// Games whose wrapper points to another Checkpoint path, repaired when Steam is closed (on app
/// start). Returns how many were repaired.
pub fn repair(steam_root: &Path) -> Result<usize> {
    let info = states(steam_root, &[])?;
    let old: Vec<u32> = info.games.iter().filter(|(_, g)| g.state == "old").map(|(id, _)| *id).collect();
    if old.is_empty() || info.steam_running {
        return Ok(0);
    }
    let r = set(steam_root, &old, true)?;
    log::info!("Launch options: {} game(s) pointed to an old Checkpoint path, repaired", r.changed);
    Ok(r.changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXE: &str = "/home/u/Apps/Checkpoint.AppImage";

    #[test]
    fn merges_and_unmerges() {
        let cases = [
            ("", "\"/home/u/Apps/Checkpoint.AppImage\" run --appid 7 -- %command%"),
            ("~/.lsfg %command%", "\"/home/u/Apps/Checkpoint.AppImage\" run --appid 7 -- ~/.lsfg %command%"),
            ("PROTON_LOG=1 DXVK_HUD=fps gamemoderun %command% -dx11", "PROTON_LOG=1 DXVK_HUD=fps \"/home/u/Apps/Checkpoint.AppImage\" run --appid 7 -- gamemoderun %command% -dx11"),
            ("VAR=1 %command%", "VAR=1 \"/home/u/Apps/Checkpoint.AppImage\" run --appid 7 -- %command%"),
            ("-dx11 -skipintro", "\"/home/u/Apps/Checkpoint.AppImage\" run --appid 7 -- %command% -dx11 -skipintro"),
        ];
        for (orig, merged) in cases {
            assert_eq!(merge(orig, EXE, 7), merged, "merge {orig:?}");
            assert_eq!(state_of(merged, EXE, 7), "on");
            assert_eq!(state_of(orig, EXE, 7), "off");
            // Merging twice changes nothing
            assert_eq!(merge(merged, EXE, 7), merged);
        }
        assert_eq!(unmerge(cases[1].1), "~/.lsfg %command%");
        assert_eq!(unmerge(cases[2].1), "PROTON_LOG=1 DXVK_HUD=fps gamemoderun %command% -dx11");
        assert_eq!(unmerge(cases[0].1), "");
        // Without %command% originally: equivalent, not identical (the stored original fixes that)
        assert_eq!(unmerge(cases[4].1), "%command% -dx11 -skipintro");
    }

    #[test]
    fn finds_old_paths_and_windows_paths() {
        let old = "\"/home/u/Downloads/Checkpoint_0.7.0_amd64.AppImage\" run --appid 7 -- ~/.lsfg %command%";
        assert_eq!(state_of(old, EXE, 7), "old");
        assert_eq!(merge(old, EXE, 7), "\"/home/u/Apps/Checkpoint.AppImage\" run --appid 7 -- ~/.lsfg %command%");
        let win = "\"C:\\Program Files\\Checkpoint\\checkpoint.exe\" run --appid 7 -- %command%";
        assert_eq!(state_of(win, "C:\\Program Files\\Checkpoint\\checkpoint.exe", 7), "on");
        assert_eq!(state_of(win, "C:\\Program Files\\Checkpoint\\checkpoint.exe", 8), "old");
        assert_eq!(unmerge(win), "");
        // Another program named like a word in the options is not the wrapper
        assert_eq!(state_of("mangohud %command% run", EXE, 7), "off");
    }

    fn sample(nl: &str) -> String {
        [
            "\"UserLocalConfigStore\"",
            "{",
            "\t\"Software\"",
            "\t{",
            "\t\t\"Valve\"",
            "\t\t{",
            "\t\t\t\"Steam\"",
            "\t\t\t{",
            "\t\t\t\t\"apps\"",
            "\t\t\t\t{",
            "\t\t\t\t\t\"4115450\"",
            "\t\t\t\t\t{",
            "\t\t\t\t\t\t\"LastPlayed\"\t\t\"1762000000\"",
            "\t\t\t\t\t\t\"LaunchOptions\"\t\t\"~/.lsfg %command%\"",
            "\t\t\t\t\t}",
            "\t\t\t\t\t\"10\"",
            "\t\t\t\t\t{",
            "\t\t\t\t\t\t\"LastPlayed\"\t\t\"1\"",
            "\t\t\t\t\t}",
            "\t\t\t\t}",
            "\t\t\t\t\"Other\"\t\t\"x \\\"y\\\" \\\\z\"",
            "\t\t\t}",
            "\t\t}",
            "\t}",
            "}",
            "",
        ]
        .join(nl)
    }

    #[test]
    fn edits_only_the_value() {
        for nl in ["\n", "\r\n"] {
            let text = sample(nl);
            let all = read_all(&text).unwrap();
            assert_eq!(all[&4115450], "~/.lsfg %command%");
            assert_eq!(all[&10], "");

            let v = "\"C:\\Apps\\checkpoint.exe\" run --appid 4115450 -- ~/.lsfg %command%";
            let out = set_in(&text, 4115450, v).unwrap();
            assert_eq!(read_all(&out).unwrap()[&4115450], v);
            assert!(out.contains("\"LaunchOptions\"\t\t\"\\\"C:\\\\Apps\\\\checkpoint.exe\\\" run --appid 4115450 -- ~/.lsfg %command%\""));
            // Only that value changed
            let before = text.find("~/.lsfg").unwrap() - 1;
            assert_eq!(&out[..before], &text[..before]);
            assert!(out.ends_with(&text[text.find("\"10\"").unwrap() - 5..]));

            // A game without LaunchOptions, and a game with no entry yet
            let out = set_in(&text, 10, "%command% -dx11").unwrap();
            assert!(out.contains(&format!("\t\t\t\t\t{{{nl}\t\t\t\t\t\t\"LaunchOptions\"\t\t\"%command% -dx11\"{nl}\t\t\t\t\t\t\"LastPlayed\"")));
            let out = set_in(&out, 99, "x").unwrap();
            let all = read_all(&out).unwrap();
            assert_eq!((all[&10].as_str(), all[&99].as_str(), all[&4115450].as_str()), ("%command% -dx11", "x", "~/.lsfg %command%"));
            assert_eq!(crate::vdf::parse(&out).get("UserLocalConfigStore").unwrap().get("Software").unwrap().get("Valve").unwrap().get("Steam").unwrap().str("Other"), Some("x \"y\" \\z"));
        }
    }

    /// Read-only check on this PC's real file: `cargo test launch_real -- --ignored --nocapture`.
    /// Writing every game's value back unchanged must give the same bytes.
    #[test]
    #[ignore]
    fn launch_real() {
        let cfg = crate::config::Config::load();
        let root = crate::steam::find_root(cfg.steam_root_override.as_deref()).expect("Steam");
        let (a, path) = account(&root).expect("account");
        let text = fs::read_to_string(&path).expect("localconfig.vdf");
        let all = read_all(&text).expect("parse");
        println!("account {} · {} bytes · {} apps", a.account_id, text.len(), all.len());
        let mut n = 0;
        for (id, v) in &all {
            if !v.is_empty() {
                n += 1;
                println!("  {id}: {v}");
                assert_eq!(set_in(&text, *id, v).unwrap(), text, "appid {id}");
            }
        }
        println!("{n} game(s) with launch options round-trip byte for byte");
        let info = states(&root, &[]).unwrap();
        println!("steam running: {}", info.steam_running);
    }

    #[test]
    fn refuses_odd_files() {
        assert!(set_in("\"UserLocalConfigStore\" { }", 1, "x").is_err());
        assert!(parse("\"a\" { \"b\" ").is_err());
        assert!(read_all("garbage {").is_err());
    }
}
