# Checkpoint

**English** · [Tiếng Việt](README.vi.md)

[![Latest release](https://img.shields.io/github/v/release/fr0stf0x/checkpoint)](https://github.com/fr0stf0x/checkpoint/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/fr0stf0x/checkpoint/total)](https://github.com/fr0stf0x/checkpoint/releases)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20me%20a%20coffee-fr0stf0x-FFDD00?logo=buymeacoffee&logoColor=black)](https://buymeacoffee.com/fr0stf0x)

Back up and restore Steam save games between your own PCs (Windows and Linux, including Bazzite) through your own Google Drive. Works while Steam runs in offline mode, where Steam Cloud is unavailable.

Download: [latest release](https://github.com/fr0stf0x/checkpoint/releases/latest) · Website and setup guide: see `site/`

## Features

- Lists installed Steam games and the Steam accounts on the machine, with game artwork and account avatars taken from Steam's local cache (works offline). Games show as a cover-art grid or a detailed list (toggle next to the search box), in a dark theme with icons.
- Finds save files using the [ludusavi-manifest](https://github.com/mtkennerly/ludusavi-manifest) (PCGamingWiki data, about 20,000 games), plus Steam Cloud's local `userdata/<account>/<appid>/remote` folder.
- Backup and restore per game, or for all games at once with a preview of what will run and what will be skipped.
- Compares by content hash, not file dates. Tells apart "local is newer", "cloud is newer" and real conflicts, and warns before either button would overwrite newer progress.
- Keeps the last 10 versions per game in the cloud. Takes a local snapshot before every restore.
- Refuses to back up or restore while the game is running. Flags games whose saves are tied to a SteamID.
- Saves made on Windows restore into the Proton prefix on Linux, and the other way round.
- Move a game's save from one Steam account to another (for example when you have to switch accounts): copies the account's save folder to the new account, fixes `steam_autocloud.vdf`, optionally replaces the SteamID inside save files, then lets you test in game and **keep** or **undo** in one click. The original account's files are never touched. Restore can also write a backup into a different account.
- Built-in rclone setup: detects the OS, downloads the official rclone with SHA-256 verification, and connects Google Drive with your own OAuth client ID.
- In-app updates: at startup the app asks GitHub for the latest release and shows what's new. **Update and restart** downloads it, checks its signature and installs it (Windows installer, or replaces the AppImage file in place). Nothing is installed until you click. Turn the check off in **Settings → Updates**. A Linux binary not started from the AppImage only gets a link to the download page.

## Languages

The app is available in English and Vietnamese. It follows the system language by default; change it in **Settings → Language**. Translations live in [`src-tauri/locales/`](src-tauri/locales) (`en.yml`, `vi.yml`) and are used by both the Rust backend ([rust-i18n](https://crates.io/crates/rust-i18n)) and the UI. To add a language, copy `en.yml`, translate it, and add it to `LANGUAGES` in `config.rs` and `LOCALE_FILES` in `main.rs`; the unit tests check that every key exists. Logs are always written in English.

The website (`site/`) uses [i18next](https://www.i18next.com/) with `site/locales/<lang>.json`.

## Reporting bugs

- The app writes a log to `%LOCALAPPDATA%\Checkpoint\logs\checkpoint.log` (Linux: `~/.local/share/Checkpoint/logs/checkpoint.log`), rotated at 2 MB. Home folder paths are replaced with `~`; tokens, client secrets and Steam account names are never logged.
- When something fails, the app shows an error dialog with **Create issue on GitHub**. It opens a new issue in your browser with version, OS, the error and the end of the log prefilled (SteamIDs masked). No GitHub credentials are stored in the app: you sign in to GitHub in the browser and press Submit.
- The **Report bug / Feedback** button in the sidebar opens the bug report or feature request form, and can copy the log or open the log folder.

## How it works

- Reads `libraryfolders.vdf` and `appmanifest_*.acf` for installed games, and `loginusers.vdf` for accounts.
- Each file is stored under a logical path such as `win/winAppData/...`, `any/base/...` or `linux/xdgData/...`. On Linux, `win/*` paths are mapped into `steamapps/compatdata/<appid>/pfx/drive_c/users/steamuser/...`.
- Status is computed from the SHA-256 of the save content and the hashes recorded at the last sync on this machine (`state.json`).
- Before each restore, the current local save is zipped to `%LOCALAPPDATA%\Checkpoint\snapshots` (`~/.local/share/Checkpoint/snapshots` on Linux). The last 5 are kept.

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

rclone's shared client ID is heavily rate limited and is being retired by Google. With your own client ID each operation takes about 3 seconds instead of more than 30. Creating one is free and takes 5–10 minutes. The step-by-step guide is on the website (`site/index.html`, section *Connect Google Drive*). The short version:

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

Tags containing `-` (for example `v0.2.0-beta.1`) are marked as pre-releases. The in-app updater ignores pre-releases.

The installers are signed for the in-app updater, and the workflow also publishes `latest.json`, which the app reads from `releases/latest/download/latest.json`. One-time setup:

1. `cargo install tauri-cli --version "^2" --locked`, then `cargo tauri signer generate -w ~/.tauri/checkpoint.key` (choose a password).
2. Put the contents of `~/.tauri/checkpoint.key.pub` into `plugins.updater.pubkey` in `src-tauri/tauri.conf.json` (the test `updater_pubkey_is_set` fails until you do).
3. Add the repository secrets `TAURI_SIGNING_PRIVATE_KEY` (contents of `~/.tauri/checkpoint.key`) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The workflow stops if the key is missing.

Keep a backup of the private key. If it is lost, installed copies cannot verify new releases and must be updated by hand once with a new key.

## Website

`site/` is a static site (home with setup guide, Privacy Policy, Terms of Service). The home page is a single page translated with i18next (English and Vietnamese, picked from `?lng=`, the last choice, or the browser language; `/en` and `/vi` redirect to it). Deploy it to Vercel with `site` as the Root Directory. Stars, download count and the download buttons come live from the GitHub API.

## Support

Checkpoint is free and open source. If it saved your progress, you can [buy me a coffee](https://buymeacoffee.com/fr0stf0x) ☕. Bug reports and ideas on [GitHub Issues](https://github.com/fr0stf0x/checkpoint/issues) help just as much.

## Known limitations

- A save tied to a SteamID may only work on that Steam account. Moving it to another account works for many games but not all (encrypted saves that check the account inside cannot be fixed).
- On Linux, run the game once through Proton before restoring a Windows backup, so the Proton prefix exists.
- Saves stored in the Windows Registry are not supported yet.
- Games missing from the manifest cannot be added manually yet.

## Roadmap

See [TODO.md](TODO.md). Highlights: a launch wrapper (`checkpoint run -- %command%`), a "playing on PC X" lock, custom game entries, game and account artwork, and a redesigned website.
