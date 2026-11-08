# Changelog

All notable changes to Checkpoint are listed here, newest first.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow [Semantic Versioning](https://semver.org/).
Each release's section is also shown at the top of its GitHub Release notes.

## [Unreleased]

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

[Unreleased]: https://github.com/fr0stf0x/checkpoint/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/fr0stf0x/checkpoint/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/fr0stf0x/checkpoint/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/fr0stf0x/checkpoint/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/fr0stf0x/checkpoint/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/fr0stf0x/checkpoint/releases/tag/v0.1.0
