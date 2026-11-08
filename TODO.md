# TODO

Planned work, not scheduled yet. When an item ships, tick it and add it to `CHANGELOG.md` under `[Unreleased]`.

## App

### Look and feel
- [x] Show Steam account avatars (read from Steam's local cache, e.g. `config/avatarcache/<SteamID64>.png`; no network needed in offline mode).
- [x] Show game artwork (Steam's local library cache, e.g. `appcache/librarycache/<appid>/` header/capsule images; fall back to a placeholder).
- [x] Use icons throughout so screens are less text-heavy (status pills, buttons, sidebar, dialogs). Pick one icon set and bundle it locally (the app works offline).
- [x] Modern UI with a gaming vibe (cover-art game grid, darker theme, stronger visual hierarchy), keeping both languages and keyboard focus states.
- [x] Installer and app have no proper logo: use the Checkpoint logo from the website (`site/icon.png`, `site/logo-120.png`) to generate every icon size (`cargo tauri icon`), and check the NSIS installer, the window/taskbar icon and the AppImage icon.
- [x] Show the app version next to the logo in the sidebar, so it is clear which version is running.
- [x] Google client ID/secret fields: once configured, show the saved values (not only the green border), masked by default with a show/hide toggle like a password field. Never write them to the log.
- [x] Controller navigation (gamepad API: D-pad/stick moves focus, A confirms, B goes back), so the app is usable in Bazzite/SteamOS Gaming Mode.

### Features
- [x] Launch wrapper `checkpoint run -- %command%` for Steam launch options: restore before playing, back up after quitting.
- [ ] "Currently playing on PC X" lock (`lock.json` in the cloud) to prevent conflicts.
- [ ] Custom game entries (games missing from the manifest, non-Steam games, emulators).
- [ ] Scheduled background backup and a tray icon.
- [ ] Saves stored in the Windows Registry (Windows ↔ Proton `user.reg`).
- [x] Restore a local snapshot from the UI.
- [x] Keep one `rclone rcd` process running instead of starting rclone for every call (~3 s per call today).
- [x] "Add to Steam library" button: add Checkpoint as a non-Steam game (`userdata/<id>/config/shortcuts.vdf`, Steam must be closed), so it can be opened from Gaming Mode.
- [x] "Delete everything in the cloud" button (Settings), behind a warning dialog that names the backup location and needs a tick box before the Delete button works.
- [x] Per game: "Delete cloud backup" button, behind a warning dialog (cannot be undone; local saves are not touched; other PCs lose this backup).
- [x] Per game: "Report a problem" button that opens a prefilled GitHub issue with the game name, appid, status, save paths and the log lines for that game.
- [x] Logs: keep ~3 rotated files instead of 1 (size-based rotation stays; per-run or per-day files are not worth it, the `starting` line already separates runs).
- [x] Logs: tag game-related log lines with the appid, so a per-game report only includes that game's lines (plus recent errors) and fits GitHub's ~8 KB URL limit.
- [x] "Open log folder" button next to "Report a bug", so users can attach the full log file to the issue when the prefilled excerpt is not enough. (Already in the Report dialog and every error dialog.)

### Release / housekeeping
- [x] Update the notes of the v0.1.0–v0.3.0 GitHub Releases with their `CHANGELOG.md` sections (newer releases get them automatically).
- [x] Test the AppImage and the Proton-prefix restore on a real Bazzite machine.

## Website

- [x] Redesign the landing page to be more attractive: modern, gaming vibe.
- [x] Show the problem and how the app solves it (Steam offline mode → no Steam Cloud → Checkpoint syncs saves through your own Google Drive; switching Steam accounts).
- [x] Take screenshots of the app (both languages if possible) and put them on the landing page.
- [x] Move the guides to their own pages/paths (for example `/guide`, `/guide/google-drive`, `/guide/switch-account`), keeping the landing page short. Keep `/privacy` and `/terms` (used by the Google consent screen) and redirects for old anchors.
