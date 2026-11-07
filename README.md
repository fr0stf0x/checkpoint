# Checkpoint

**English** · [Tiếng Việt](README.vi.md)

Back up and restore Steam save games between your own PCs (Windows and Linux, including Bazzite) through your own Google Drive. Works while Steam runs in offline mode, where Steam Cloud is unavailable.

Download: [latest release](https://github.com/fr0stf0x/checkpoint/releases/latest) · Website and setup guide: see `site/`

## Features

- Lists installed Steam games and the Steam accounts on the machine.
- Finds save files using the [ludusavi-manifest](https://github.com/mtkennerly/ludusavi-manifest) (PCGamingWiki data, about 20,000 games), plus Steam Cloud's local `userdata/<account>/<appid>/remote` folder.
- Backup and restore per game, or for all games at once with a preview of what will run and what will be skipped.
- Compares by content hash, not file dates. Tells apart "local is newer", "cloud is newer" and real conflicts, and warns before either button would overwrite newer progress.
- Keeps the last 10 versions per game in the cloud. Takes a local snapshot before every restore.
- Refuses to back up or restore while the game is running. Flags games whose saves are tied to a SteamID.
- Saves made on Windows restore into the Proton prefix on Linux, and the other way round.
- Built-in rclone setup: detects the OS, downloads the official rclone with SHA-256 verification, and connects Google Drive with your own OAuth client ID.

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

- **Google Drive via rclone (recommended, Windows and Linux):** in Settings, click **Install rclone** if needed, paste your Google **Client ID** and **Client secret**, then **Connect Google Drive**. The app runs `rclone config create gdrive drive scope=drive.file client_id=… client_secret=…`, opens the browser for Google sign-in, and uses `gdrive:Checkpoint`. Do this on every PC with the same client ID and the same Google account.
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

Tags containing `-` (for example `v0.2.0-beta.1`) are marked as pre-releases.

## Website

`site/` is a static site (home, setup guide, Privacy Policy, Terms of Service) in Vietnamese (`/`) and English (`/en`). Deploy it to Vercel with `site` as the Root Directory. Stars, download count and the download buttons come live from the GitHub API.

## Known limitations

- A save tied to a SteamID only works on that Steam account.
- On Linux, run the game once through Proton before restoring a Windows backup, so the Proton prefix exists.
- Saves stored in the Windows Registry are not supported yet.
- Games missing from the manifest cannot be added manually yet.

## Roadmap

- Launch wrapper `checkpoint run -- %command%` (restore before playing, back up after quitting)
- "Currently playing on PC X" lock
- Custom game entries
- Restore a local snapshot from the UI
