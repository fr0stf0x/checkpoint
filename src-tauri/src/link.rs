//! Linked Steam accounts: several accounts that belong to one player (for example a library
//! shared through Steam Family, played as account A on one PC and account B on another).
//!
//! The groups live in the cloud `index.json` (`linked_accounts`), so every PC sees them, also a PC
//! where one of the accounts was never signed in. When the account signed in on this PC is in a
//! group, a game's save "belongs to the player":
//!
//! - Backup and status only look at this account's files (plus folders with no account in their
//!   path); the other accounts' folders on this PC are left out.
//! - Status compares a normalized hash: paths and contents (steam_autocloud.vdf, readable IDs in
//!   files of this account's folders) are hashed as if they belonged to the group's canonical
//!   account, so the same save hashes the same on every PC. The canonical account is stored per
//!   group (`link_canon` in the index) and kept when accounts join or leave, so adding an account
//!   never changes the hashes; groups from before that use the lowest SteamID64.
//! - Restore writes a backup made by another account of the group into this account (folder
//!   names, steam_autocloud.vdf, and readable IDs in files, same length).
//!
//! "One player" mode (`one_player` in the index): every Steam account that uses this backup
//! location is the same player. The first time a PC's signed-in account is not in the player's
//! group, the app asks once whether it is the player: yes adds it to the group, no stores it in
//! `not_player` (it then syncs on its own, as without links). Until then the launch wrapper does
//! not sync, so another person's account on a shared PC never overwrites the player's save.
//!
//! Encrypted saves that check the account can't be helped; the UI says to test those in game.

use crate::migrate::{self, Ids};
use crate::paths::LocalScan;
use serde::{Deserialize, Serialize};

/// One account of a linked group. `name` is only for display on PCs that don't know the account.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkedAccount {
    pub sid64: String,
    #[serde(default)]
    pub name: String,
}

fn lowest(group: &[LinkedAccount]) -> Option<String> {
    group.iter().map(|a| a.sid64.clone()).min_by(|a, b| a.len().cmp(&b.len()).then(a.cmp(b)))
}

/// A group's canonical account: the one stored in `canon`, else the lowest SteamID64.
pub fn canon_of(group: &[LinkedAccount], canon: &[String]) -> Option<String> {
    group.iter().find(|a| canon.contains(&a.sid64)).map(|a| a.sid64.clone()).or_else(|| lowest(group))
}

/// This PC's signed-in account and the rest of its group.
#[derive(Debug, Clone)]
pub struct Link {
    pub me: Ids,
    pub others: Vec<Ids>,
    /// The account hashes are normalized to (see `canon_of`)
    pub canon: Ids,
}

/// The link for this PC, if its signed-in account is in a group of two or more.
pub fn for_pc(groups: &[Vec<LinkedAccount>], canon: &[String], me: Option<Ids>) -> Option<Link> {
    let me = me?;
    let group = groups.iter().find(|g| g.iter().any(|a| a.sid64 == me.sid64))?;
    let mut ids: Vec<Ids> = group.iter().filter_map(|a| Ids::parse(&a.sid64).ok()).collect();
    ids.sort_by(|a, b| a.sid64.len().cmp(&b.sid64.len()).then(a.sid64.cmp(&b.sid64)));
    ids.dedup();
    if ids.len() < 2 {
        return None;
    }
    let c = canon_of(group, canon)?;
    let canon = ids.iter().find(|i| i.sid64 == c)?.clone();
    let others = ids.into_iter().filter(|i| i.sid64 != me.sid64).collect();
    Some(Link { me, others, canon })
}

/// Put `members` into one group, taking them out of any other group (groups left with one account
/// go). The new group keeps the canonical account of a group it takes accounts from (so hashes
/// made before stay valid), unless `prefer` (a member) is given.
pub fn set_group(groups: &mut Vec<Vec<LinkedAccount>>, canon: &mut Vec<String>, members: Vec<LinkedAccount>, prefer: Option<&str>) {
    let is_member = |s: &str| members.iter().any(|m| m.sid64 == s);
    let keep = prefer
        .filter(|p| is_member(p))
        .map(str::to_string)
        .or_else(|| groups.iter().filter_map(|g| canon_of(g, canon)).find(|c| is_member(c)))
        .or_else(|| lowest(&members));
    for g in groups.iter_mut() {
        g.retain(|a| !is_member(&a.sid64));
    }
    groups.retain(|g| g.len() >= 2);
    if members.len() >= 2 {
        canon.retain(|c| !is_member(c));
        canon.extend(keep);
        groups.push(members);
    }
    tidy(groups, canon);
}

/// Remove the group that contains `sid64`.
pub fn remove_group(groups: &mut Vec<Vec<LinkedAccount>>, canon: &mut Vec<String>, sid64: &str) {
    groups.retain(|g| !g.iter().any(|a| a.sid64 == sid64));
    tidy(groups, canon);
}

/// One stored canonical account per group, and none for groups that are gone.
fn tidy(groups: &[Vec<LinkedAccount>], canon: &mut Vec<String>) {
    let mut out = Vec::new();
    for g in groups {
        if let Some(c) = g.iter().find(|a| canon.contains(&a.sid64)) {
            out.push(c.sid64.clone());
        }
    }
    *canon = out;
}

/// The player's group in "one player" mode: the group with the player's account, or the player
/// alone while no other account has joined.
pub fn player_group(groups: &[Vec<LinkedAccount>], player: &LinkedAccount) -> Vec<LinkedAccount> {
    groups.iter().find(|g| g.iter().any(|a| a.sid64 == player.sid64)).cloned().unwrap_or_else(|| vec![player.clone()])
}

/// Turning "one player" mode on from a PC signed in as `me`: the player's group is `me`'s linked
/// group, else the only linked group there is (`me` joins it), else `me` alone. The group's
/// canonical account stays and is stored. Returns the player (that canonical account).
pub fn turn_on(groups: &mut Vec<Vec<LinkedAccount>>, canon: &mut Vec<String>, me: LinkedAccount) -> LinkedAccount {
    let mine = groups.iter().find(|g| g.iter().any(|a| a.sid64 == me.sid64)).cloned();
    let group = match (mine, groups.as_slice()) {
        (Some(g), _) => Some(g),
        (None, [only]) => {
            let mut g = only.clone();
            let c = canon_of(&g, canon);
            g.push(me.clone());
            set_group(groups, canon, g.clone(), c.as_deref());
            Some(g)
        }
        _ => None,
    };
    let Some(g) = group else { return me };
    let c = canon_of(&g, canon).unwrap_or_else(|| me.sid64.clone());
    // Stored, so it stays canonical whatever account joins later
    if !canon.contains(&c) {
        canon.push(c.clone());
    }
    g.into_iter().find(|a| a.sid64 == c).unwrap_or(me)
}

/// "One player" mode is on and this PC's account has not said whether it is the player.
pub fn asks(player: Option<&LinkedAccount>, groups: &[Vec<LinkedAccount>], not_player: &[LinkedAccount], me: Option<&str>) -> bool {
    let (Some(p), Some(me)) = (player, me) else { return false };
    !player_group(groups, p).iter().any(|a| a.sid64 == me) && !not_player.iter().any(|a| a.sid64 == me)
}

fn tied(logical: &str, who: &Ids) -> bool {
    migrate::remap_logical(logical, who, who).is_some()
}

impl Link {
    /// The file is in another group account's folder (not this PC's account).
    pub fn is_others(&self, logical: &str) -> bool {
        !tied(logical, &self.me) && self.others.iter().any(|o| tied(logical, o))
    }

    /// The files this PC backs up: everything but the other group accounts' folders.
    pub fn filter(&self, mut scan: LocalScan) -> LocalScan {
        scan.files.retain(|f| !self.is_others(&f.logical));
        scan
    }

    /// Whether a file's content may carry this account's ID (its own folder, or Steam's owner file).
    pub fn content_needs_normalizing(&self, logical: &str, is_autocloud: bool) -> bool {
        self.me != self.canon && (is_autocloud || tied(logical, &self.me))
    }

    /// Logical path as if the save belonged to the canonical account.
    pub fn normalize_path(&self, logical: &str) -> String {
        migrate::remap_logical(logical, &self.me, &self.canon).unwrap_or_else(|| logical.to_string())
    }

    /// File content as if it belonged to the canonical account.
    pub fn normalize_content(&self, mut data: Vec<u8>, is_autocloud: bool) -> Vec<u8> {
        if is_autocloud {
            migrate::rewrite_autocloud(&data, &self.me, &self.canon)
        } else {
            migrate::replace_ids(&mut data, &self.me, &self.canon);
            data
        }
    }

    /// The group account a backup belongs to, when it is another account than this PC's: restore
    /// then writes it into this account. None when the backup has this account, several group
    /// accounts, or none.
    pub fn source_of(&self, version_ids: &[String]) -> Option<Ids> {
        let mut found: Vec<Ids> = version_ids.iter().filter_map(|i| Ids::parse(i).ok()).collect();
        found.dedup_by(|a, b| a.sid64 == b.sid64);
        if found.iter().any(|i| i.sid64 == self.me.sid64) {
            return None;
        }
        let mine: Vec<&Ids> = self.others.iter().filter(|o| found.iter().any(|f| f.sid64 == o.sid64)).collect();
        match mine.as_slice() {
            [only] => Some((*only).clone()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::SaveFile;

    const A: &str = "76561199698261377";
    const B: &str = "76561198012345678";

    fn acc(s: &str) -> LinkedAccount {
        LinkedAccount { sid64: s.into(), name: String::new() }
    }

    fn file(logical: &str) -> SaveFile {
        SaveFile { logical: logical.into(), real: logical.into(), size: 1, mtime_ms: 0 }
    }

    #[test]
    fn link_for_pc_and_canonical_account() {
        let groups = vec![vec![acc(A), acc(B)]];
        let l = for_pc(&groups, &[], Ids::parse(A).ok()).unwrap();
        assert_eq!(l.me.sid64, A);
        assert_eq!(l.others.len(), 1);
        assert_eq!(l.canon.sid64, B, "lowest SteamID64 is canonical");
        assert!(for_pc(&groups, &[], Ids::parse("76561197960265729").ok()).is_none());
        assert!(for_pc(&groups, &[], None).is_none());
    }

    #[test]
    fn filter_drops_other_accounts_folders_only() {
        let l = for_pc(&[vec![acc(A), acc(B)]], &[], Ids::parse(A).ok()).unwrap();
        let b_acct = Ids::parse(B).unwrap().acct;
        let scan = LocalScan {
            files: vec![
                file(&format!("win/winLocalAppData/Game/Steam/{A}/save.sav")),
                file(&format!("win/winLocalAppData/Game/Steam/{B}/save.sav")),
                file(&format!("win/steam/userdata/{b_acct}/123/remote/x.sav")),
                file("win/winDocuments/Game/shared.sav"),
            ],
            roots: vec![],
        };
        let kept: Vec<String> = l.filter(scan).files.into_iter().map(|f| f.logical).collect();
        assert_eq!(kept.len(), 2);
        assert!(kept.iter().all(|k| !k.contains(B) && !k.contains(&b_acct)));
    }

    #[test]
    fn normalized_paths_and_contents_match_across_accounts() {
        let groups = vec![vec![acc(A), acc(B)]];
        let on_a = for_pc(&groups, &[], Ids::parse(A).ok()).unwrap();
        let on_b = for_pc(&groups, &[], Ids::parse(B).ok()).unwrap();
        assert_eq!(on_a.normalize_path(&format!("win/x/Steam/{A}/s.sav")), on_b.normalize_path(&format!("win/x/Steam/{B}/s.sav")));
        let (ia, ib) = (Ids::parse(A).unwrap(), Ids::parse(B).unwrap());
        let vdf = |acct: &str| format!("\"\"\n{{\n\t\"accountid\"\t\t\"{acct}\"\n}}\n").into_bytes();
        assert_eq!(on_a.normalize_content(vdf(&ia.acct), true), on_b.normalize_content(vdf(&ib.acct), true));
        let blob = |s: &str| [b"head".as_slice(), s.as_bytes(), b"tail"].concat();
        assert_eq!(on_a.normalize_content(blob(A), false), on_b.normalize_content(blob(B), false));
    }

    #[test]
    fn source_of_a_backup() {
        let l = for_pc(&[vec![acc(A), acc(B)]], &[], Ids::parse(A).ok()).unwrap();
        let b_acct = Ids::parse(B).unwrap().acct;
        assert_eq!(l.source_of(&[B.into(), b_acct]).map(|i| i.sid64), Some(B.into()));
        assert!(l.source_of(&[A.into()]).is_none());
        assert!(l.source_of(&[A.into(), B.into()]).is_none());
        assert!(l.source_of(&[]).is_none());
    }

    #[test]
    fn set_and_remove_groups() {
        let c = "76561197960265729";
        let mut g = vec![vec![acc(A), acc(c)]];
        let mut canon = Vec::new();
        set_group(&mut g, &mut canon, vec![acc(A), acc(B)], None);
        assert_eq!(g, vec![vec![acc(A), acc(B)]], "A moved out of its old group, which had one account left");
        // The old group's canonical account was C (lowest), not a member: the new group's lowest
        assert_eq!(canon, vec![B.to_string()]);
        remove_group(&mut g, &mut canon, B);
        assert!(g.is_empty() && canon.is_empty());
    }

    #[test]
    fn canonical_account_stays_when_accounts_join() {
        let c = "76561197960265729"; // lower than A and B
        let mut g = vec![vec![acc(A), acc(B)]];
        let mut canon = Vec::new();
        assert_eq!(for_pc(&g, &canon, Ids::parse(A).ok()).unwrap().canon.sid64, B);
        // C joins: B (canonical so far) stays canonical although C is lower
        set_group(&mut g, &mut canon, vec![acc(A), acc(B), acc(c)], None);
        assert_eq!(canon, vec![B.to_string()]);
        assert_eq!(for_pc(&g, &canon, Ids::parse(c).ok()).unwrap().canon.sid64, B);
        // prefer wins
        set_group(&mut g, &mut canon, vec![acc(A), acc(B), acc(c)], Some(A));
        assert_eq!(canon, vec![A.to_string()]);
        assert_eq!(for_pc(&g, &canon, Ids::parse(c).ok()).unwrap().canon.sid64, A);
    }

    #[test]
    fn turning_one_player_on() {
        let c = "76561197960265729";
        // PC C, not linked, A+B linked elsewhere: C joins A+B, B (lowest of A, B) stays canonical
        let (mut g, mut canon) = (vec![vec![acc(A), acc(B)]], Vec::new());
        let p = turn_on(&mut g, &mut canon, acc(c));
        assert_eq!(p.sid64, B);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].len(), 3);
        assert_eq!(canon, vec![B.to_string()]);
        assert_eq!(for_pc(&g, &canon, Ids::parse(c).ok()).unwrap().canon.sid64, B);
        // PC A, already linked: same group, nothing added
        let (mut g, mut canon) = (vec![vec![acc(A), acc(B)]], Vec::new());
        assert_eq!(turn_on(&mut g, &mut canon, acc(A)).sid64, B);
        assert_eq!(g, vec![vec![acc(A), acc(B)]]);
        // No links yet: the player is this PC's account alone
        let (mut g, mut canon) = (Vec::new(), Vec::new());
        assert_eq!(turn_on(&mut g, &mut canon, acc(c)).sid64, c);
        assert!(g.is_empty());
        // Two other groups: no guess, this PC's account alone
        let d = "76561198099999999";
        let (mut g, mut canon) = (vec![vec![acc(A), acc(B)], vec![acc(d), acc("76561198088888888")]], Vec::new());
        assert_eq!(turn_on(&mut g, &mut canon, acc(c)).sid64, c);
        assert_eq!(g.len(), 2);
    }

    #[test]
    fn one_player_asks_once() {
        let c = "76561197960265729";
        let player = acc(A);
        let g = vec![vec![acc(A), acc(B)]];
        assert!(!asks(None, &g, &[], Some(c)), "mode off");
        assert!(!asks(Some(&player), &g, &[], Some(A)));
        assert!(!asks(Some(&player), &g, &[], Some(B)));
        assert!(asks(Some(&player), &g, &[], Some(c)));
        assert!(!asks(Some(&player), &g, &[acc(c)], Some(c)), "answered no");
        assert!(!asks(Some(&player), &[], &[], Some(A)), "the player alone, no group yet");
        assert!(asks(Some(&player), &[], &[], Some(B)));
        assert!(!asks(Some(&player), &g, &[], None), "no account signed in");
    }
}
