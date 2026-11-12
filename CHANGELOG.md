# Changelog

All notable changes to Checkpoint are listed here, newest first.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow [Semantic Versioning](https://semver.org/).
Each release's section is also shown at the top of its GitHub Release notes.

## [Unreleased]

### Added
- Steam launch option: a system notification tells you what happened around the game session: the cloud save was restored, the save was backed up, a sync failed (with the reason), or a save was left for you to decide in the app because both sides changed. Turn it off in Settings → Backup ("Notify when the Steam launch option syncs a game").

### Added
- A **Quit** button in the sidebar, below Report bug / Feedback, to close the app in Gaming Mode, where there is no window close button. While a backup or restore is running it shows a message instead of quitting.

### Changed
- Backup and restore show their progress instead of a bare spinner: a list of the game's save files (file name and a shortened folder path), each with a spinner while it is packed or written and a check when done, plus a row for the upload or download with its percentage. A restore lists its three steps up front (keep the save on this PC for Undo, download the backup, write the save files), and each one turns from a spinner to a check as it finishes. Backing up or restoring several games shows the same list for each game in turn.

### Fixed
- The logo in the sidebar now matches the app icon (blue, as on the taskbar and the website), and the version badge sits under the app name instead of squeezing the logo.

## [0.7.2] - 2026-11-08

### Fixed
- Launch option (`checkpoint run …`): after you quit the game, Checkpoint no longer keeps waiting for it. The wrapper counted itself as the game still running, so the automatic backup never happened, Steam kept showing the game as running and the app showed it as **Running**.
- If the launch wrapper is stopped abruptly, the rclone process it started now stops with it, so Steam no longer stays on **Stopping** for the game.
- The game list refreshes by itself when you come back to the app, so a game you just closed no longer shows as **Running** and its new save shows up without pressing reload.

## [0.7.0] - 2026-11-08

### Added
- Undo a restore. Every restore keeps the save it overwrites on this PC; the message after a restore has an **Undo** button, and a game's **⋯** dialog now has **Earlier saves**, listing each kept save with when it was last played, what replaced it and its size. **Go back to this save** puts it back exactly as it was, and keeps the current save as a new entry so you can change your mind. Earlier saves can also be deleted, or opened in their folder.

### Changed
- Better controller navigation. Each game is one stop: the D-pad moves from game to game in order (right at the end of a row goes on to the next row instead of jumping to the search box) and no longer lands on each game's small buttons. **A** on a game opens its menu (Back up, Restore, Earlier saves, Select, Details), **X** selects it. Closing a dialog returns to the game you were on, the sidebar shows the button hints while a controller is in use, and a D-pad press that Steam sends both as a gamepad and as an arrow key moves only once.
- Up to 10 earlier saves are kept per game instead of 5, and the same save is never kept twice.

## [0.6.0] - 2026-11-08

### Added
- Select several games for backup or restore: hover a game and tick its box (or click covers once something is selected; the list view has a select-all box). The header buttons become **Back up N games / Restore N games**.
- The Back up all / Restore all preview lets you untick games, has Select all / Select none, and shows each game's size and the total to upload or download.
- Game cards and the list show the size of the local save and of the latest cloud backup.
- Released under the MIT license (`LICENSE`).
- Controller navigation for Gaming Mode on Bazzite / SteamOS (and any gamepad): the D-pad or left stick moves between buttons, **A** presses, **B** closes a dialog or goes back to the sidebar, **LB / RB** switch screens. Arrow keys, Enter and Esc work the same way.
- **Add Checkpoint to Steam** in Settings: adds the app to your Steam library as a non-Steam game, so you can start it from the library and from Gaming Mode. Quit Steam first; Steam's previous list is kept as `shortcuts.vdf.checkpoint-bak`.
- A **⋯** button on every game opens its details (save folders, local and cloud versions) with **Delete cloud backup** and **Report a problem**. The report prefills the game, its status and save folders, and only that game's log lines plus warnings and errors.
- **Delete everything in the cloud** in Settings, behind a confirmation. Deleting never touches the saves on your PCs.
- The app version is shown next to the logo.
- Automatic sync from Steam's launch options: copy the line from a game's **⋯** dialog into **Properties → Launch options**. Checkpoint restores before the game starts when the cloud is newer and backs up after you quit when this PC is newer; conflicts are left for you, and the game always starts.
- Once Google Drive is connected, Settings shows the saved client ID and secret, hidden until you press the eye button, so you can copy them to your other PCs.

### Changed
- The log keeps three old files instead of one, and errors about a game are logged with its AppID.
- Google Drive is faster: the app keeps one rclone running in the background instead of starting it for every file (a backup and restore round trip took about 40% less time in testing).

### Fixed
- The Folder field no longer shows in Settings while Google Drive is selected as the backup location.

## [0.5.1] - 2026-11-08

### Fixed
- Restoring a backup made with another Steam account no longer loses the save for games that keep it in a shared folder such as the game's install folder (Slay the Spire, for example). Steam marks those folders with the owning account in `steam_autocloud.vdf` and cleared them when a different account started the game; restore now gives them to the account signed in on this PC ([#2](https://github.com/fr0stf0x/checkpoint/issues/2)).

## [0.5.0] - 2026-11-08

### Added
- Game artwork in the game list and avatars on the Steam accounts page, read from Steam's local cache (no download, works in offline mode). Games or accounts without a cached image show their first letter.

### Changed
- New look: a dark theme, icons on buttons, status badges and the sidebar, and a cover-art grid for games. The detailed list is still there (toggle next to the search box), and the app remembers your choice. On narrow windows the sidebar shrinks to icons. The light theme was removed.
- Full set of app icons generated from the Checkpoint logo, used by the Windows installer, the window and taskbar, and the AppImage.

## [0.4.0] - 2026-11-08

### Added
- In-app updates. At startup Checkpoint checks GitHub for a newer release and shows what's new; **Update and restart** downloads it, verifies its signature and installs it (Windows installer, or the AppImage file is replaced in place). Nothing is installed until you click, and updating waits while a backup, restore or move is running. Turn the check off or check by hand in **Settings → Updates**.

## [0.3.1] - 2026-11-08

### Added
- **Copy link** and **Open in browser** in the Google Drive sign-in dialog, for when rclone cannot open a browser itself (for example Flatpak Firefox on Bazzite). Paste the link into a browser on the same PC.
- This changelog. Release notes now start with the changes of that version, followed by the install instructions.

## [0.3.0] - 2026-11-07

### Added
- English and Vietnamese interface. The app follows the system language by default; change it in **Settings → Language**. Translations live in `src-tauri/locales/` and are shared by the backend and the UI.
- Log file on each PC (`%LOCALAPPDATA%\Checkpoint\logs\checkpoint.log`, `~/.local/share/Checkpoint/logs/checkpoint.log` on Linux), rotated at 2 MB. Home folder paths are replaced with `~`; tokens, client secrets and Steam account names are never logged. Logs are always in English.
- Error dialog with **Create issue on GitHub**: opens a new issue in the browser with version, OS, the error and the end of the log prefilled (SteamIDs masked). No GitHub credentials are stored in the app.
- **Report bug / Feedback** button in the sidebar, with Copy log and Open log folder.
- GitHub issue templates for bug reports and feature requests.
- Buy Me a Coffee links in the README and on the website, and a Sponsor button on the GitHub repository.

### Changed
- The website is now a single page translated with i18next (English and Vietnamese, picked from `?lng=`, the last choice or the browser language). `/en` and `/vi` redirect to it.
- Code comments, log messages, the release workflow and issue template titles are in English.
- Vietnamese UI text addresses the user as "bạn".

## [0.2.0] - 2026-11-07

### Added
- Move a game's save from one Steam account to another (for example when you have to switch accounts). A 3-step wizard (choose, prepare, test in game) copies the source account's save to the target account, rewrites `steam_autocloud.vdf`, and can optionally replace the SteamID inside save files. After testing in game, **Keep** or **Undo** in one click; undo removes the copy (including files the game created during the test) and restores the target account's previous files. The source account's files are never modified.
- Saves are classified before moving: not account-bound, ID only in the folder name, ID inside the file, or encrypted (must be tested in game).
- Restore can write a backup into a different Steam account. When a move is kept, the account pair is stored in the cloud so other PCs restore into the new account by default.
- Entry points: **Switch account** on account-bound games, a button on each Steam account card, and the account selector in the Restore dialog.
- English README (`README.md`) with the Vietnamese version in `README.vi.md`; English page on the website.

## [0.1.0] - 2026-11-07

First release.

### Added
- Lists installed Steam games (all libraries) and the Steam accounts on the PC, read from local files, so it works with Steam in offline mode.
- Finds save files with the ludusavi manifest (PCGamingWiki data, about 20,000 games) plus Steam Cloud's local `userdata/<account>/<appid>/remote` folder.
- Backup and restore per game, and Back up all / Restore all with a preview of what will run and what will be skipped.
- Status by content hash (SHA-256) and the last sync on this PC: in sync, local is newer, cloud is newer, conflict, never synced. Warnings in both directions: before a restore would overwrite newer local progress and before a backup would hide newer cloud progress.
- Keeps the last 10 versions per game in the cloud; any version can be restored. A local snapshot is taken before every restore.
- Blocks backup and restore while the game is running. Flags games whose saves are tied to a SteamID.
- Windows saves restore into the Proton prefix on Linux (Bazzite) and the other way round.
- Graphics/settings files are excluded by default (optional).
- Storage on Google Drive through rclone, or any folder (for example Google Drive for Desktop).
- Built-in rclone setup: detects the OS and CPU, downloads the official rclone from downloads.rclone.org with SHA-256 verification (no admin rights), and connects Google Drive with your own OAuth client ID (`drive.file` scope).
- GitHub Actions release workflow: pushing a `vX.Y.Z` tag runs the tests and publishes a Windows installer and a Linux AppImage.
- Website (`site/`) with download buttons, live GitHub stats, a setup guide for creating a Google client ID, Privacy Policy and Terms of Service.

[Unreleased]: https://github.com/fr0stf0x/checkpoint/compare/v0.7.2...HEAD
[0.7.2]: https://github.com/fr0stf0x/checkpoint/compare/v0.7.0...v0.7.2
[0.7.0]: https://github.com/fr0stf0x/checkpoint/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/fr0stf0x/checkpoint/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/fr0stf0x/checkpoint/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/fr0stf0x/checkpoint/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/fr0stf0x/checkpoint/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/fr0stf0x/checkpoint/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/fr0stf0x/checkpoint/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/fr0stf0x/checkpoint/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/fr0stf0x/checkpoint/releases/tag/v0.1.0
