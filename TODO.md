# TODO

Planned work, not scheduled yet. When an item ships, tick it and add it to `CHANGELOG.md` under `[Unreleased]`.

## App

### Look and feel
- [x] Show Steam account avatars (read from Steam's local cache, e.g. `config/avatarcache/<SteamID64>.png`; no network needed in offline mode).
- [x] Show game artwork (Steam's local library cache, e.g. `appcache/librarycache/<appid>/` header/capsule images; fall back to a placeholder).
- [x] Use icons throughout so screens are less text-heavy (status pills, buttons, sidebar, dialogs). Pick one icon set and bundle it locally (the app works offline).
- [x] Modern UI with a gaming vibe (cover-art game grid, darker theme, stronger visual hierarchy), keeping both languages and keyboard focus states.
- [x] Installer and app have no proper logo: use the Checkpoint logo from the website (`site/icon.png`, `site/logo-120.png`) to generate every icon size (`cargo tauri icon`), and check the NSIS installer, the window/taskbar icon and the AppImage icon.

### Features
- [ ] Launch wrapper `checkpoint run -- %command%` for Steam launch options: restore before playing, back up after quitting.
- [ ] "Currently playing on PC X" lock (`lock.json` in the cloud) to prevent conflicts.
- [ ] Custom game entries (games missing from the manifest, non-Steam games, emulators).
- [ ] Scheduled background backup and a tray icon.
- [ ] Saves stored in the Windows Registry (Windows ↔ Proton `user.reg`).
- [ ] Restore a local snapshot from the UI.
- [ ] Keep one `rclone rcd` process running instead of starting rclone for every call (~3 s per call today).

### Release / housekeeping
- [ ] Update the notes of the v0.1.0–v0.3.0 GitHub Releases with their `CHANGELOG.md` sections (newer releases get them automatically).
- [ ] Test the AppImage and the Proton-prefix restore on a real Bazzite machine.

## Website

- [x] Redesign the landing page to be more attractive: modern, gaming vibe.
- [x] Show the problem and how the app solves it (Steam offline mode → no Steam Cloud → Checkpoint syncs saves through your own Google Drive; switching Steam accounts).
- [x] Take screenshots of the app (both languages if possible) and put them on the landing page.
- [x] Move the guides to their own pages/paths (for example `/guide`, `/guide/google-drive`, `/guide/switch-account`), keeping the landing page short. Keep `/privacy` and `/terms` (used by the Google consent screen) and redirects for old anchors.
