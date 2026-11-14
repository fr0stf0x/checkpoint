# Checkpoint – project notes for Claude

Read this before working on the project. README.md explains what the app does and how to build it; this file records **why** things are the way they are, the design decisions, and the working conventions. Planned work lives in `TODO.md`, released changes in `CHANGELOG.md`.

**If `CLAUDE.local.md` exists, read it too.** It holds the maintainer's personal preferences and local machine notes (it is gitignored, so it only exists on their machines):

@CLAUDE.local.md

## Purpose

The maintainer plays Steam games on three PCs (two Windows 11, one Bazzite). Steam only runs in offline mode there, so Steam Cloud is unavailable. Checkpoint replaces it: it backs up and restores save games through the user's own Google Drive and works on Windows and Linux.

Original requirements (all shipped in 0.1.0): run on Windows and Bazzite; list installed games and Steam accounts; connect one Google Drive folder; backup/restore per game and for all games; warn on restore when the local save is newer. Later additions: in-app rclone install and Google client ID setup, release builds on tags, a bilingual website, moving saves between Steam accounts, logging and bug reports, English/Vietnamese UI.

## Feasibility and design decisions

- **Not possible:** using Steam Cloud itself while offline; guaranteeing a save works on another Steam account (many games embed the SteamID); finding saves for 100% of games; syncing while a game is running; merging two saves.
- **Limited:** saves in the Windows Registry (Windows ↔ Proton `user.reg`) are not supported; native-Linux builds of a game may use other paths and formats than the Windows build, so on Bazzite prefer running games through Proton; in Gaming Mode the controller drives the UI through the Gamepad API (or arrow keys from Steam Input), but typing needs Steam's on-screen keyboard.
- **Status by content hash, not file dates.** Clocks drift and unzipping changes dates. Each game's status compares the SHA-256 of its save files with the hashes recorded at the last sync on this PC (`state.json`), which distinguishes local newer / cloud newer / conflict / never synced. Warnings go both ways: restoring over newer local progress and backing up over newer cloud progress. A forced backup over cloud progress this PC lacks (status cloud / conflict / diverged) marks the replaced version `kept` in `index.json`: not counted in `keep_versions`, never pruned, until "Stop keeping" in History (`sync::prune`, `sync::unkeep`).
- **`steam_autocloud.vdf` owner on restore.** Steam clears save folders whose `steam_autocloud.vdf` names another account when a game starts. Restore therefore sets the owner of shared folders (no account ID in the path) to the account signed in on this PC (most recent in `loginusers.vdf`, or the remap target); folders named after an account keep their owner (`sync::fix_autocloud_owners`, issue #2).
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
| `storage.rs`, `rclone.rs`, `rcd.rs` | Folder / rclone storage backends; rclone detection, install (SHA-256 checked), Drive connect; one `rclone rcd` per process (localhost, random password, CLI fallback, stopped on exit and before reconnecting) |
| `run.rs` | Launch wrapper `checkpoint run --appid N -- %command%`: restore if cloud newer, run the game, wait until nothing runs from its folder, back up if local newer; no window |
| `launch.rs` | Writes the wrapper into a game's Steam launch options (`localconfig.vdf`, edited in place, Steam closed); merges with existing options (`VAR=1` first, then the wrapper, then other wrappers like `~/.lsfg`), stores the original in `launch_options.json`, repairs old Checkpoint paths on start |
| `notify.rs`, `prompt.rs` | Launch wrapper notifications (notify-rust); the conflict question as `checkpoint prompt <file>` (small window, answer written back to the file) |
| `link.rs` | Linked Steam accounts (one player): which files this PC syncs, normalized hash, which account a backup is restored from |
| `updater.rs` | In-app updates (tauri-plugin-updater): check `latest.json`, signed download and install, restart |
| `shortcuts.rs` | "Add to Steam library": binary `shortcuts.vdf` read/write (byte-exact for other shortcuts, Steam must be closed) |
| `images.rs` | Avatars and game artwork from Steam's local caches, served to the UI by the `steamimg` URI scheme |
| `config.rs` | `config.json` (device, backend, language…) and `state.json` (last sync hashes) |
| `logger.rs`, `report.rs`, `i18n.rs` | File log (3 rotated files; game lines carry `appid=<n>`, see `blocking_for`), prefilled GitHub issue URL (per-game reports only take that game's lines), `tr!` macro (translated errors with English for logs) |
| `src-tauri/locales/{en,vi}.yml` | All user-facing text for the backend (`err.*`, `msg.*`) and the UI (`ui.*`) |
| `ui/` | Plain HTML/JS frontend (no Node, no bundler) |
| `site/` | Website (Vercel, clean URLs): landing page, `/guide`, `/guide/google-drive`, `/guide/switch-account`, `/privacy`, `/terms`; shared header/footer and i18next in `main.js`; screenshots in `shots/` |
| `.github/workflows/release.yml` | Tag `vX.Y.Z` → tests, Windows NSIS + Linux AppImage, GitHub Release |

## Moving saves between Steam accounts (A → B)

Classification decides what the app can do:
- Not account-bound → nothing to do.
- SteamID/AccountID only in a folder name (or `userdata/<accountid>`) → copy into B's folder and rewrite `steam_autocloud.vdf` (`"accountid"`). Works.
- ID inside the file in readable form (text, UTF-16 or little-endian u64) → same-length replacement, marked experimental (checksums may break).
- Encrypted/compressed file → unknown whether the game checks the account; copy, test in game, undo if needed.
- Server-side saves → impossible.

Implementation rules (`migrate.rs`): always **copy**, never modify or delete A's files. The move record (`<data>/migrations/<id>.json`) is written **before** copying so a half-done move can be undone; B's overwritten files are captured in `<id>.zip` and newly created folders are recorded, so undo also removes files the game created during the test. One pending move per game. "Keep" stores `account_aliases[A] = B` in the cloud `index.json`; the Restore dialog then defaults to writing A's backups into B (`remap_from`/`remap_to`).

### Linked accounts (one player, several accounts)

For a player who uses account A on one PC and B on another (a library shared through Steam Family), moving saves by hand every day doesn't work. `linked_accounts` in the cloud `index.json` groups accounts; when the account signed in on a PC (most recent in `loginusers.vdf`) is in a group (`link.rs`):
- `sync_scan` leaves out the other group accounts' folders on this PC, so a backup holds only this account's save (plus folders with no account in their path).
- `save_hash` hashes paths and contents as if they belonged to the group's canonical account (see below): folder names, `steam_autocloud.vdf`, readable IDs in this account's files. The same save hashes the same on every PC, so status stays "In sync" across accounts. Without a group it is exactly `hash_scan`, so nothing changes for others.
- Restore remaps a backup made by another group account into this one (`Link::source_of`), including same-length ID replacement inside files (`patch_in_file`). The snapshot before restoring still holds every local file.
- The canonical account is stored per group (`link_canon`) and kept when accounts join or leave (`set_group` keeps the canonical account of a group it takes members from); groups without a stored one use the lowest SteamID64. So a new account joining never changes the other PCs' hashes.
- **One player mode** (`one_player`, `not_player` in `index.json`): the whole backup location is one player. Turning it on picks this PC's group, else the only existing group (this account joins), else this account alone (`link::turn_on`). A PC whose signed-in account is in neither the player's group nor `not_player` asks once (banner); "yes" joins the group with the player's account as canonical, "no" goes to `not_player`. While unanswered, the launch wrapper does not sync (another person's account on a shared PC must not overwrite the player's save). Older app versions drop the unknown index fields when they save the index, so every PC must be updated.
- After linking, each bound game needs one new backup (old hashes were not normalized). Encrypted saves that check the account can't be fixed; the UI says to test in game.

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
- **When the app changes, update the docs in the same change:** `README.md` and `README.vi.md` (features, usage, limitations), the website (`site/index.html` and both `site/locales/*.json`: features, guide, notes), `CHANGELOG.md` under `[Unreleased]`, and `TODO.md` if an item shipped (remove it). A user-visible feature is not done until the README and landing page describe it in both languages.
- After changing `ui/`, rebuild the Rust app: Tauri embeds the frontend in the binary.
- Never print or commit rclone tokens, client secrets or the contents of `rclone.conf`. Logs replace the home folder with `~`; exported logs mask SteamIDs.
- Do not add `Co-Authored-By: Claude` (or any AI attribution) to commits or PRs.

## Testing

- `cargo test` in `src-tauri` (unit tests; CI runs them in parallel, so tests that change the global locale take `LOCALE_LOCK`).
- On a real machine (ignored by default): `cargo test smoke -- --ignored --nocapture` (scan Steam, back up into a temp folder), `cargo test drive_real -- --ignored --nocapture` (real backup to the configured storage), `cargo test install_real -- --ignored --nocapture` (download rclone), `cargo test preview_real -- --ignored --nocapture` (read-only account-move preview). Never restore over real saves while testing.
- UI without the app: `CHECKPOINT_DUMP=<dir> cargo test dump_ui_strings -- --ignored` writes the UI strings to JSON; serve a copy of `ui/` with a `mock.js` that fakes `window.__TAURI__.core.invoke` (including `get_strings`).

## Releases and changelog

- Release = push a `vX.Y.Z` tag; the workflow builds and publishes. Ask the maintainer before pushing or tagging unless they said so.
- **Test versions first:** a tag with a suffix (`vX.Y.Z-beta.N`) becomes a GitHub pre-release with the `[Unreleased]` changelog section as notes (no section of its own needed). Regular users never see it (the updater endpoint and the website skip pre-releases); PCs with **Settings → Updates → test versions** on get it through the in-app updater. Test on the real PCs, then release `vX.Y.Z` as usual.
- **Every release needs a changelog entry, in English:**
  - While working, add user-facing changes under `## [Unreleased]` in `CHANGELOG.md` (Keep a Changelog: Added / Changed / Fixed / Removed). Write for users, not a list of code changes.
  - Before tagging: rename `## [Unreleased]` to `## [X.Y.Z] - YYYY-MM-DD`, add a new empty `## [Unreleased]` above it, update the compare links at the bottom, commit, then tag.
  - The workflow puts that version's section under "What's new", **above** the install instructions in the GitHub Release notes, and fails if the section is missing.
- When a `TODO.md` item ships, remove it from `TODO.md` and add it to the changelog. `TODO.md` lists open work only, by priority (P1 next up, P2 no rush, P3 only on real need).
