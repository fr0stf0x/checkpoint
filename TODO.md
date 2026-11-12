# TODO

Open work, highest priority first. When an item ships, remove it here and add it to `CHANGELOG.md` under `[Unreleased]` (shipped work lives in the changelog, not here).

- **P1**: next up.
- **P2**: worth doing, no rush.
- **P3**: only if a real need shows up.

## P1

### Write the Steam launch option for the user
Today the launch option must be copied from each game's **⋯** dialog into Steam, per game, per PC, per account. Write it into `userdata/<accountid>/config/localconfig.vdf` (`UserLocalConfigStore → Software → Valve → Steam → apps → <appid> → "LaunchOptions"`, text VDF) instead:
- [ ] Per-game switch, a "Turn on for N games" button with a preview of what changes, a "Remove all" button, and an "Auto sync" badge on game cards.
- [ ] Steam must be closed (it rewrites the file on exit), as for "Add to Steam library". On Bazzite this means Desktop mode.
- [ ] Edit only the `LaunchOptions` line of that game (never rewrite the whole file), escape `"` and `\`, keep `localconfig.vdf.checkpoint-bak`, write to a temp file, read it back, then rename.
- [ ] Merge with existing options: insert the wrapper before `%command%` (`gamemoderun %command%`, `VAR=1 %command%`); without `%command%`, put the wrapper first and keep the rest as game arguments (`-dx11`). Store the original so removing restores it exactly.
- [ ] A moved or deleted Checkpoint breaks every game that uses it: on start, repair launch options that point to an old Checkpoint path (AppImage moved to another folder).
- [ ] The launch wrapper must never keep a game from starting: catch panics before and after the game.
- [ ] Measure how long the wrapper delays a game start when offline, and cap the cloud check with a timeout.
- [ ] Default to the account signed in on this PC; let the user pick other accounts.

### Check 0.8.0 on real machines
- [ ] Bazzite: launch wrapper notifications and the conflict window in Desktop mode and Gaming Mode (does the window show, does the controller work).
- [ ] Windows installed build (NSIS): the toast shows the Checkpoint name and icon.
- [ ] Controller navigation on a handheld in Gaming Mode (ROG Xbox Ally X): right from the last game in a row goes to the next game, focus moves between games and not their small buttons; dialogs, the hint panel, the backup/restore progress dialog and the Quit button.
- [ ] A forced backup over a conflicting cloud version marks that version **Kept** in History (Phantom Blade Zero is in conflict now).
- [ ] Move Phantom Blade Zero's save to another Steam account and test it in game (encrypted save, account binding unknown).

## P2

- [ ] Setup code for a new PC: "Code for another PC" on a configured PC bundles the Google client ID, client secret and backup folder into one string; pasting it on a new PC installs rclone, fills those in and leaves only the Google sign-in. Warn not to post the code publicly (it contains the client secret); never log it.
- [ ] Custom game entries (games missing from the manifest, non-Steam games, emulators): name and save folder, stored so the same entry works on Windows and in the Proton prefix on Linux.

## P3

- [ ] "Currently playing on PC X" lock (`lock.json` in the cloud) to prevent conflicts. Needs stale-lock handling (PC turned off, offline) and a manual unlock. Less needed now that the launch wrapper asks on a conflict.
- [ ] Scheduled background backup and a tray icon. Mostly covered by the launch wrapper backing up after every session.
- [ ] Saves stored in the Windows Registry (Windows ↔ Proton `user.reg`). Few games, easy to get wrong; wait for a concrete game.
