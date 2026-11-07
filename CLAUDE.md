# Checkpoint – project notes for Claude

Read this before working on the project. README.md explains what the app does and how to build it; this file records **why** things are the way they are, the design decisions, and the working conventions. Planned work lives in `TODO.md`, released changes in `CHANGELOG.md`.

**If `CLAUDE.local.md` exists, read it too.** It holds the maintainer's personal preferences and local machine notes (it is gitignored, so it only exists on their machines):

@CLAUDE.local.md

## Purpose

The maintainer plays Steam games on three PCs (two Windows 11, one Bazzite). Steam only runs in offline mode there, so Steam Cloud is unavailable. Checkpoint replaces it: it backs up and restores save games through the user's own Google Drive and works on Windows and Linux.

Original requirements (all shipped in 0.1.0): run on Windows and Bazzite; list installed games and Steam accounts; connect one Google Drive folder; backup/restore per game and for all games; warn on restore when the local save is newer. Later additions: in-app rclone install and Google client ID setup, release builds on tags, a bilingual website, moving saves between Steam accounts, logging and bug reports, English/Vietnamese UI.

## Feasibility and design decisions

- **Not possible:** using Steam Cloud itself while offline; guaranteeing a save works on another Steam account (many games embed the SteamID); finding saves for 100% of games; syncing while a game is running; merging two saves.
- **Limited:** saves in the Windows Registry (Windows ↔ Proton `user.reg`) are not supported; native-Linux builds of a game may use other paths and formats than the Windows build, so on Bazzite prefer running games through Proton; the UI is hard to use in Bazzite's Gaming Mode with a controller.
- **Status by content hash, not file dates.** Clocks drift and unzipping changes dates. Each game's status compares the SHA-256 of its save files with the hashes recorded at the last sync on this PC (`state.json`), which distinguishes local newer / cloud newer / conflict / never synced. Warnings go both ways: restoring over newer local progress and backing up over newer cloud progress.
- **Portable logical paths.** Backups store files as `<flavor>/<token>/<rest>` (e.g. `win/winAppData/...`), so a Windows backup restores into the Proton prefix on Linux and back.
- **Google Drive through rclone** (Drive for Desktop has no Linux client). rclone's shared client ID is heavily rate limited (~37 s per call) and being retired, so users create their own OAuth client (Desktop app, scope `drive.file`) and must **publish it to "In production"**; in Testing mode tokens expire after 7 days. With `drive.file`, different client IDs cannot see each other's files, so every PC must use the same client ID and Google account.
- After Google sign-in, the browser showing "127.0.0.1 refused to connect" is **expected**: rclone received the code and closed its temporary server.

## Code map

| Path | Role |
|---|---|
| `src-tauri/src/main.rs` | Tauri commands, `blocking()` wrapper (runs work off the UI thread, logs errors in English), UI strings (`get_strings`) |
| `steam.rs`, `vdf.rs` | Steam install, libraries, installed games, accounts (`loginusers.vdf`) |
| `manifest.rs` | Downloads the ludusavi manifest and caches the Steam subset as JSON |
| `paths.rs` | Manifest rules → real files; logical path ↔ real path (incl. Proton prefix); path safety checks |
| `sync.rs` | List with status, backup, restore (with optional account remap), cloud `index.json`, snapshots |
| `migrate.rs` | Moving a save from Steam account A to B: preview, apply, undo, keep |
| `storage.rs`, `rclone.rs` | Folder / rclone storage backends; rclone detection, install (SHA-256 checked), Drive connect |
| `config.rs` | `config.json` (device, backend, language…) and `state.json` (last sync hashes) |
| `logger.rs`, `report.rs`, `i18n.rs` | File log, prefilled GitHub issue URL, `tr!` macro (translated errors with English for logs) |
| `src-tauri/locales/{en,vi}.yml` | All user-facing text for the backend (`err.*`, `msg.*`) and the UI (`ui.*`) |
| `ui/` | Plain HTML/JS frontend (no Node, no bundler) |
| `site/` | Website (Vercel): i18next single page, `/privacy`, `/terms` |
| `.github/workflows/release.yml` | Tag `vX.Y.Z` → tests, Windows NSIS + Linux AppImage, GitHub Release |

## Moving saves between Steam accounts (A → B)

Classification decides what the app can do:
- Not account-bound → nothing to do.
- SteamID/AccountID only in a folder name (or `userdata/<accountid>`) → copy into B's folder and rewrite `steam_autocloud.vdf` (`"accountid"`). Works.
- ID inside the file in readable form (text, UTF-16 or little-endian u64) → same-length replacement, marked experimental (checksums may break).
- Encrypted/compressed file → unknown whether the game checks the account; copy, test in game, undo if needed.
- Server-side saves → impossible.

Implementation rules (`migrate.rs`): always **copy**, never modify or delete A's files. The move record (`<data>/migrations/<id>.json`) is written **before** copying so a half-done move can be undone; B's overwritten files are captured in `<id>.zip` and newly created folders are recorded, so undo also removes files the game created during the test. One pending move per game. "Keep" stores `account_aliases[A] = B` in the cloud `index.json`; the Restore dialog then defaults to writing A's backups into B (`remap_from`/`remap_to`).

Example: Phantom Blade Zero (appid 4115450) keeps saves in `<base>/ProjectPBZ/Saved/SaveGames/Steam/<SteamID64>/` (`PBZSaveGame.sav`, `1RealTime/`, `2Hourly/`, `3Daily/`, `steam_autocloud.vdf`). The `.sav` header is `PBZS` with ~8 bits/byte entropy and no readable SteamID → "test in game" class.

## Conventions

- **Code, comments, commit messages, logs, changelog and this file are in English.** No hard-coded user-facing strings.
- **User-facing text** lives in `src-tauri/locales/en.yml` and `vi.yml` (rust-i18n):
  - backend errors: `tr!("err.*")` (macro in `i18n.rs`; translates and remembers the English text so `blocking()` logs errors in English);
  - success messages: `t!("msg.*")`;
  - UI: the `ui.*` subtree, served by `get_strings`, used through `t()` / `h()` in `ui/app.js` (`h` escapes inserted values for HTML). Plurals use `_one` / `_other` keys picked by `%{n}`.
  - Add every key to **both** files; the tests `locale_files_have_same_keys` and `ui_code_keys_exist` catch missing keys.
  - Vietnamese UI text addresses the user as "bạn".
- Keep `README.md` (English) and `README.vi.md` in sync. The website uses i18next with `site/locales/en.json` and `vi.json`.
- After changing `ui/`, rebuild the Rust app: Tauri embeds the frontend in the binary.
- Never print or commit rclone tokens, client secrets or the contents of `rclone.conf`. Logs replace the home folder with `~`; exported logs mask SteamIDs.
- Do not add `Co-Authored-By: Claude` (or any AI attribution) to commits or PRs.

## Testing

- `cargo test` in `src-tauri` (unit tests; CI runs them in parallel, so tests that change the global locale take `LOCALE_LOCK`).
- On a real machine (ignored by default): `cargo test smoke -- --ignored --nocapture` (scan Steam, back up into a temp folder), `cargo test drive_real -- --ignored --nocapture` (real backup to the configured storage), `cargo test install_real -- --ignored --nocapture` (download rclone), `cargo test preview_real -- --ignored --nocapture` (read-only account-move preview). Never restore over real saves while testing.
- UI without the app: `CHECKPOINT_DUMP=<dir> cargo test dump_ui_strings -- --ignored` writes the UI strings to JSON; serve a copy of `ui/` with a `mock.js` that fakes `window.__TAURI__.core.invoke` (including `get_strings`).

## Releases and changelog

- Release = push a `vX.Y.Z` tag; the workflow builds and publishes. Ask the maintainer before pushing or tagging unless they said so.
- **Every release needs a changelog entry, in English:**
  - While working, add user-facing changes under `## [Unreleased]` in `CHANGELOG.md` (Keep a Changelog: Added / Changed / Fixed / Removed). Write for users, not a list of code changes.
  - Before tagging: rename `## [Unreleased]` to `## [X.Y.Z] - YYYY-MM-DD`, add a new empty `## [Unreleased]` above it, update the compare links at the bottom, commit, then tag.
  - The workflow puts that version's section under "What's new", **above** the install instructions in the GitHub Release notes, and fails if the section is missing.
- When a `TODO.md` item ships, tick it and add it to the changelog.
