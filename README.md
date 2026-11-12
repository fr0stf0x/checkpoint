# Checkpoint

**English** · [Tiếng Việt](README.vi.md)

[![Latest release](https://img.shields.io/github/v/release/fr0stf0x/checkpoint)](https://github.com/fr0stf0x/checkpoint/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Downloads](https://img.shields.io/github/downloads/fr0stf0x/checkpoint/total)](https://github.com/fr0stf0x/checkpoint/releases)
[![Ko-fi](https://img.shields.io/badge/Ko--fi-fr0stf0x-FF5E5B?logo=kofi&logoColor=white)](https://ko-fi.com/fr0stf0x)

Back up and restore Steam save games between your own PCs (Windows and Linux, including Bazzite) through your own Google Drive. Works while Steam runs in offline mode, where Steam Cloud is unavailable.

Download: [latest release](https://github.com/fr0stf0x/checkpoint/releases/latest) · Website and setup guide: see `site/`

## Features

- Lists installed Steam games and the Steam accounts on the machine, with game artwork and account avatars taken from Steam's local cache (works offline). Games show as a cover-art grid or a detailed list (toggle next to the search box), in a dark theme with icons.
- Finds save files using the [ludusavi-manifest](https://github.com/mtkennerly/ludusavi-manifest) (PCGamingWiki data, about 20,000 games), plus Steam Cloud's local `userdata/<account>/<appid>/remote` folder.
- Backup and restore per game, for several games or for all at once. Hover a game (or focus it with the controller) and tick its box to select it; the header buttons then read **Back up N games / Restore N games**. The preview lists what will run and what will be skipped, lets you untick games, and shows each game's size and the total. Cards and the list show the size of the local save and of the cloud backup. While a backup or restore runs, a list shows each save file (name and shortened folder) with a spinner while it is packed or written and a check when done, plus the upload or download with its percentage.
- Compares by content hash, not file dates. Tells apart "local is newer", "cloud is newer" and real conflicts, and warns before either button would overwrite newer progress.
- Keeps the last 10 versions per game in the cloud. When a backup overwrites progress this PC didn't have (a conflict, or **Back up anyway** over a newer cloud version, also from the launch option), the replaced cloud version is marked **Kept**: it doesn't count towards the 10 and is never removed automatically. **Stop keeping** in History lets it go.
- Undo a restore: before a restore overwrites a save, the save on this PC is kept (the last 10 per game). Click **Undo** in the message after the restore, or open the game's **⋯ → Earlier saves** to see each copy (when it was last played, what replaced it, size) and go back to it in one click. Going back keeps the current save too, so you can change your mind again.
- Refuses to back up or restore while the game is running. Flags games whose saves are tied to a SteamID.
- Saves made on Windows restore into the Proton prefix on Linux, and the other way round.
- Move a game's save from one Steam account to another (for example when you have to switch accounts): copies the account's save folder to the new account, fixes `steam_autocloud.vdf`, optionally replaces the SteamID inside save files, then lets you test in game and **keep** or **undo** in one click. The original account's files are never touched. Restore can also write a backup into a different account.
- Each game's **⋯** button opens its details (save folders, local and cloud versions) with **Delete cloud backup** and **Report a problem**. **Settings → Backup location → Delete everything in the cloud** removes every backup at once. Both ask for confirmation and never touch the saves on your PCs.
- **Add Checkpoint to Steam** (Settings → This PC) adds the app to your Steam library as a non-Steam game, so it opens from the library and from Gaming Mode on Bazzite / SteamOS. Quit Steam first; a copy of Steam's previous list is kept as `shortcuts.vdf.checkpoint-bak`. Gaming Mode has no window close button, so the sidebar has a **Quit** button below Report bug / Feedback.
- Automatic sync from Steam: copy the launch option from a game's **⋯** dialog (`"<path to Checkpoint>" run --appid <appid> -- %command%`) into the game's **Properties → Launch options**. Before the game starts, Checkpoint restores the cloud save if the cloud is newer; after you quit (and nothing runs from the game folder any more), it backs up if this PC's save is newer. If the save conflicts when the game starts (both sides changed, or this PC never synced), a small Checkpoint window asks which save to use: the cloud one (this PC's save is kept for Undo), this PC's (backed up over the cloud after you quit), or don't start the game. Settings can change this to always start the game or never start it. Offline, the game always starts. A system notification tells you what happened (restored, backed up, failed, or left for you to decide); turn it off in Settings. Everything is written to the log.
- Controller navigation: the D-pad or left stick moves from game to game in order (right at the end of a row goes on to the next row), **A** opens the game's menu (Back up, Restore, Earlier saves, Select, Details) or presses a button, **X** selects a game, **B** closes a dialog or goes back to the sidebar, **LB / RB** switch screens. Button hints show in the sidebar while a controller is in use. Arrow keys, Enter, Space and Esc do the same, for Steam Input layouts that send keys.
- Built-in rclone setup: detects the OS, downloads the official rclone with SHA-256 verification, and connects Google Drive with your own OAuth client ID. Once connected, Settings shows the saved client ID and secret, hidden until you press the eye button, so you can copy them to your other PCs.
- The app version is shown next to the logo.
- In-app updates: at startup the app asks GitHub for the latest release and shows what's new. **Update and restart** downloads it, checks its signature and installs it (Windows installer, or replaces the AppImage file in place). Nothing is installed until you click. Turn the check off in **Settings → Updates**, where you can also opt in to test versions (pre-releases, tags like `v0.9.0-beta.1`) for a PC you test with; everyone else only gets regular releases. A Linux binary not started from the AppImage only gets a link to the download page.

## Languages

The app is available in English and Vietnamese. It follows the system language by default; change it in **Settings → Language**. Translations live in [`src-tauri/locales/`](src-tauri/locales) (`en.yml`, `vi.yml`) and are used by both the Rust backend ([rust-i18n](https://crates.io/crates/rust-i18n)) and the UI. To add a language, copy `en.yml`, translate it, and add it to `LANGUAGES` in `config.rs` and `LOCALE_FILES` in `main.rs`; the unit tests check that every key exists. Logs are always written in English.

The website (`site/`) uses [i18next](https://www.i18next.com/) with `site/locales/<lang>.json`.

## Reporting bugs

- The app writes a log to `%LOCALAPPDATA%\Checkpoint\logs\checkpoint.log` (Linux: `~/.local/share/Checkpoint/logs/checkpoint.log`). Past 2 MB it moves to `checkpoint.1.log`; the last three old files are kept. Home folder paths are replaced with `~`; tokens, client secrets and Steam account names are never logged.
- When something fails, the app shows an error dialog with **Create issue on GitHub**. It opens a new issue in your browser with version, OS, the error and the end of the log prefilled (SteamIDs masked). No GitHub credentials are stored in the app: you sign in to GitHub in the browser and press Submit.
- The **Report bug / Feedback** button in the sidebar opens the bug report or feature request form, and can copy the log or open the log folder.
- **Report a problem** in a game's **⋯** dialog prefills the game, its status and save folders, and only that game's log lines plus warnings and errors, so the part that matters fits in the issue. If you need the full log, click **Open log folder** and drag the file into the issue.

## How it works

- Reads `libraryfolders.vdf` and `appmanifest_*.acf` for installed games, and `loginusers.vdf` for accounts.
- Each file is stored under a logical path such as `win/winAppData/...`, `any/base/...` or `linux/xdgData/...`. On Linux, `win/*` paths are mapped into `steamapps/compatdata/<appid>/pfx/drive_c/users/steamuser/...`.
- Status is computed from the SHA-256 of the save content and the hashes recorded at the last sync on this machine (`state.json`).
- With Google Drive, the app keeps one `rclone rcd` running on 127.0.0.1 (random password, stopped when the app exits) instead of starting rclone for every file, so the Drive connection stays warm. If it cannot start, the app runs rclone per call as before.
- Steam marks save folders with their owner in `steam_autocloud.vdf` and clears folders owned by another account when a game starts. On restore, shared folders (install folder, AppData, Documents) are given to the account signed in on this PC; folders named after an account keep their owner.
- Before each restore, the current local save is zipped to `%LOCALAPPDATA%\Checkpoint\snapshots\<appid>` (`~/.local/share/Checkpoint/snapshots/<appid>` on Linux) with a `checkpoint-snapshot.json` describing it. The last 10 per game are kept. Going back to one restores its files exactly (files added after it are removed) and leaves the last-sync record alone, so the game then shows "local is newer" if it has a cloud backup.

Cloud layout:

```
Checkpoint/
├── index.json                          # every version of every game, newest first
└── games/<appid>/<UTC>_<device>.zip    # save files + checkpoint-meta.json
```

## Storage

- **Google Drive via rclone (recommended, Windows and Linux):** in Settings, click **Install rclone** if needed, paste your Google **Client ID** and **Client secret**, then **Connect Google Drive**. The app runs `rclone config create gdrive drive scope=drive.file client_id=… client_secret=…`, opens the browser for Google sign-in, and uses `gdrive:Checkpoint`. If no browser opens (for example Flatpak Firefox on Bazzite), the sign-in dialog shows the link with **Copy link** and **Open in browser**: paste it into a browser on the same PC. Do this on every PC with the same client ID and the same Google account.
- **Folder:** any folder, for example `G:\My Drive\Checkpoint` with Google Drive for Desktop on Windows.

### Why your own Google client ID

rclone's shared client ID is heavily rate limited and is being retired by Google. With your own client ID each operation takes about 3 seconds instead of more than 30. Creating one is free and takes 5–10 minutes. The step-by-step guide is on the website (`/guide/google-drive`, file `site/guide/google-drive.html`). The short version:

1. Create a project in Google Cloud Console and enable the Google Drive API.
2. Google Auth Platform: External audience, then **Publish app** so the status is **In production**. In Testing mode sign-ins expire after 7 days.
3. Create an OAuth client of type **Desktop app** and copy the client ID and secret.

The `drive.file` scope only lets the app see files it created, and it is not a sensitive scope, so no Google verification is needed.

## Build

Requires Rust (MSVC toolchain on Windows). The frontend is plain HTML/JS in `ui/`, so Node is not needed for development.

```bash
cd src-tauri
cargo run              # run in dev mode
cargo test             # unit tests
cargo test smoke -- --ignored --nocapture   # scan the real Steam install, test backup into a temp folder
```

Packaging (`.exe`, AppImage) needs the Tauri CLI: `cargo install tauri-cli --version "^2" --locked`, then `cargo tauri build`. On Linux, install `webkit2gtk4.1` development packages first (on Bazzite, inside a distrobox or toolbox).

## Releases

Pushing a `vX.Y.Z` tag runs [.github/workflows/release.yml](.github/workflows/release.yml): unit tests, then a Windows NSIS installer and a Linux AppImage, published as a GitHub Release. The version number is taken from the tag.

```bash
git tag v0.2.0
git push origin v0.2.0
```

Tags containing `-` (for example `v0.2.0-beta.1`) are marked as pre-releases and use the `[Unreleased]` changelog section as notes. The in-app updater ignores pre-releases unless test versions are turned on in Settings → Updates.

The installers are signed for the in-app updater, and the workflow also publishes `latest.json`, which the app reads from `releases/latest/download/latest.json`. One-time setup:

1. `cargo install tauri-cli --version "^2" --locked`, then `cargo tauri signer generate -w ~/.tauri/checkpoint.key` (choose a password).
2. Put the contents of `~/.tauri/checkpoint.key.pub` into `plugins.updater.pubkey` in `src-tauri/tauri.conf.json` (the test `updater_pubkey_is_set` fails until you do).
3. Add the repository secrets `TAURI_SIGNING_PRIVATE_KEY` (contents of `~/.tauri/checkpoint.key`) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The workflow stops if the key is missing.

Keep a backup of the private key. If it is lost, installed copies cannot verify new releases and must be updated by hand once with a new key.

## Website

`site/` is a static site: the landing page (`index.html`), the guides (`guide/index.html` → `/guide`, `guide/google-drive.html`, `guide/switch-account.html`), and the Privacy Policy and Terms of Service used by the Google consent screen. Every page is translated with i18next (English and Vietnamese, picked from `?lng=`, the last choice, or the browser language; `/en` and `/vi` redirect to the home page). `main.js` adds the shared header and footer and sends the old home-page anchors `#guide` and `#google` to the guide pages. App screenshots live in `site/shots/` (one per language). Deploy it to Vercel with `site` as the Root Directory. Stars, download count and the download buttons come live from the GitHub API.

## Support

Checkpoint is free and open source. If it saved your progress, you can [buy me a coffee on Ko-fi](https://ko-fi.com/fr0stf0x) ☕. Bug reports and ideas on [GitHub Issues](https://github.com/fr0stf0x/checkpoint/issues) help just as much.

## Known limitations

- A save tied to a SteamID may only work on that Steam account. Moving it to another account works for many games but not all (encrypted saves that check the account inside cannot be fixed).
- On Linux, run the game once through Proton before restoring a Windows backup, so the Proton prefix exists.
- Saves stored in the Windows Registry are not supported yet.
- Games missing from the manifest cannot be added manually yet.

## Roadmap

See [TODO.md](TODO.md). Highlights: a launch wrapper (`checkpoint run -- %command%`), a "playing on PC X" lock, custom game entries, game and account artwork, and a redesigned website.

## License

[MIT](LICENSE) © fr0stf0x. Save locations come from [ludusavi-manifest](https://github.com/mtkennerly/ludusavi-manifest) (PCGamingWiki data); Google Drive access goes through [rclone](https://rclone.org/), downloaded separately.
