# TODO

Open work, highest priority first. When an item ships, remove it here and add it to `CHANGELOG.md` under `[Unreleased]` (shipped work lives in the changelog, not here).

- **P1**: next up.
- **P2**: worth doing, no rush.
- **P3**: only if a real need shows up.

## P1

### Steam launch option: what is left
- [ ] Test on Bazzite: turn on automatic sync for Phantom Blade Zero (`~/.lsfg %command%`), start it from Desktop Mode and Gaming Mode, check frame generation still works, the restore/backup notifications and the conflict window.
- [ ] Measure how long the wrapper delays a game start when offline, and cap the cloud check with a timeout.
- [ ] Let the user pick other Steam accounts on this PC (today: the account signed in most recently).

### Check 0.8.0 on real machines
- [ ] Bazzite: launch wrapper notifications and the conflict window in Desktop mode and Gaming Mode (does the window show, does the controller work).
- [ ] Windows installed build (NSIS): the toast shows the Checkpoint name and icon.
- [ ] Controller navigation on a handheld in Gaming Mode (ROG Xbox Ally X): right from the last game in a row goes to the next game, focus moves between games and not their small buttons; dialogs, the hint panel, the backup/restore progress dialog and the Quit button.
- [ ] A forced backup over a conflicting cloud version marks that version **Kept** in History (Phantom Blade Zero is in conflict now).
- [ ] Move Phantom Blade Zero's save to another Steam account and test it in game (encrypted save, account binding unknown).
- [ ] One player mode on the real PCs: turn it on from Jonsbo, then open the app on a PC with an account Checkpoint has not seen; it asks once, "yes" joins, the game shows In sync after one backup, and the launch option does not sync before the answer.
- [ ] Linked accounts on the real PCs (PC A signed in as Acc1, PC B as Acc2): link, back up each bound game once, then a full day/night cycle through the launch option; both PCs show In sync, and the game loads the save on each account (Phantom Blade Zero is encrypted: test in game).

## P2

- [ ] Setup code for a new PC: "Code for another PC" on a configured PC bundles the Google client ID, client secret and backup folder into one string; pasting it on a new PC installs rclone, fills those in and leaves only the Google sign-in. Warn not to post the code publicly (it contains the client secret); never log it.
- [ ] Custom game entries (games missing from the manifest, non-Steam games, emulators): name and save folder, stored so the same entry works on Windows and in the Proton prefix on Linux.

## P3

- [ ] "Currently playing on PC X" lock (`lock.json` in the cloud) to prevent conflicts. Needs stale-lock handling (PC turned off, offline) and a manual unlock. Less needed now that the launch wrapper asks on a conflict.
- [ ] Scheduled background backup and a tray icon. Mostly covered by the launch wrapper backing up after every session.
- [ ] Saves stored in the Windows Registry (Windows ↔ Proton `user.reg`). Few games, easy to get wrong; wait for a concrete game.
