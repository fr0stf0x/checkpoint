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
//!   account (lowest SteamID64), so the same save hashes the same on every PC.
//! - Restore writes a backup made by another account of the group into this account (folder
//!   names, steam_autocloud.vdf, and readable IDs in files, same length).
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

/// This PC's signed-in account and the rest of its group.
#[derive(Debug, Clone)]
pub struct Link {
    pub me: Ids,
    pub others: Vec<Ids>,
    /// The account hashes are normalized to: the lowest SteamID64 of the group
    pub canon: Ids,
}

/// The link for this PC, if its signed-in account is in a group of two or more.
pub fn for_pc(groups: &[Vec<LinkedAccount>], me: Option<Ids>) -> Option<Link> {
    let me = me?;
    let group = groups.iter().find(|g| g.iter().any(|a| a.sid64 == me.sid64))?;
    let mut ids: Vec<Ids> = group.iter().filter_map(|a| Ids::parse(&a.sid64).ok()).collect();
    ids.sort_by(|a, b| a.sid64.len().cmp(&b.sid64.len()).then(a.sid64.cmp(&b.sid64)));
    ids.dedup();
    if ids.len() < 2 {
        return None;
    }
    let canon = ids[0].clone();
    let others = ids.into_iter().filter(|i| i.sid64 != me.sid64).collect();
    Some(Link { me, others, canon })
}

/// Put `ids` into one group, taking them out of any other group (groups left with one account go).
pub fn set_group(groups: &mut Vec<Vec<LinkedAccount>>, members: Vec<LinkedAccount>) {
    for g in groups.iter_mut() {
        g.retain(|a| !members.iter().any(|m| m.sid64 == a.sid64));
    }
    groups.retain(|g| g.len() >= 2);
    if members.len() >= 2 {
        groups.push(members);
    }
}

/// Remove the group that contains `sid64`.
pub fn remove_group(groups: &mut Vec<Vec<LinkedAccount>>, sid64: &str) {
    groups.retain(|g| !g.iter().any(|a| a.sid64 == sid64));
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
        let l = for_pc(&groups, Ids::parse(A).ok()).unwrap();
        assert_eq!(l.me.sid64, A);
        assert_eq!(l.others.len(), 1);
        assert_eq!(l.canon.sid64, B, "lowest SteamID64 is canonical");
        assert!(for_pc(&groups, Ids::parse("76561197960265729").ok()).is_none());
        assert!(for_pc(&groups, None).is_none());
    }

    #[test]
    fn filter_drops_other_accounts_folders_only() {
        let l = for_pc(&[vec![acc(A), acc(B)]], Ids::parse(A).ok()).unwrap();
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
        let on_a = for_pc(&groups, Ids::parse(A).ok()).unwrap();
        let on_b = for_pc(&groups, Ids::parse(B).ok()).unwrap();
        assert_eq!(on_a.normalize_path(&format!("win/x/Steam/{A}/s.sav")), on_b.normalize_path(&format!("win/x/Steam/{B}/s.sav")));
        let (ia, ib) = (Ids::parse(A).unwrap(), Ids::parse(B).unwrap());
        let vdf = |acct: &str| format!("\"\"\n{{\n\t\"accountid\"\t\t\"{acct}\"\n}}\n").into_bytes();
        assert_eq!(on_a.normalize_content(vdf(&ia.acct), true), on_b.normalize_content(vdf(&ib.acct), true));
        let blob = |s: &str| [b"head".as_slice(), s.as_bytes(), b"tail"].concat();
        assert_eq!(on_a.normalize_content(blob(A), false), on_b.normalize_content(blob(B), false));
    }

    #[test]
    fn source_of_a_backup() {
        let l = for_pc(&[vec![acc(A), acc(B)]], Ids::parse(A).ok()).unwrap();
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
        set_group(&mut g, vec![acc(A), acc(B)]);
        assert_eq!(g, vec![vec![acc(A), acc(B)]], "A moved out of its old group, which had one account left");
        remove_group(&mut g, B);
        assert!(g.is_empty());
    }
}
