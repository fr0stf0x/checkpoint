//! Detects, installs and configures rclone for Google Drive.

use crate::storage::{managed_rclone_path, rclone_command, resolve_rclone};
use anyhow::{bail, Context, Result};
use crate::tr;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::process::Stdio;
use std::time::{Duration, Instant};

const DOWNLOADS: &str = "https://downloads.rclone.org";

#[derive(Debug, Serialize)]
pub struct RemoteInfo {
    pub name: String,
    pub kind: String,
    pub scope: Option<String>,
    /// true when the remote uses its own client_id (not rclone's shared one)
    pub custom_client: bool,
}

#[derive(Debug, Serialize)]
pub struct RcloneStatus {
    pub os: String,
    pub arch: String,
    pub platform: Option<String>,
    pub found: bool,
    pub bin: String,
    pub managed: bool,
    pub version: Option<String>,
    pub remote: Option<RemoteInfo>,
}

/// Platform name as used in rclone's download file names, e.g. "windows-amd64".
fn platform() -> Option<String> {
    let os = match std::env::consts::OS {
        "windows" => "windows",
        "linux" => "linux",
        "macos" => "osx",
        _ => return None,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "x86" => "386",
        _ => return None,
    };
    Some(format!("{os}-{arch}"))
}

fn version_of(bin: &str) -> Option<String> {
    let out = rclone_command(bin).arg("version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).lines().next().map(|l| l.trim().to_string())
}

/// Read one remote's config. Only the fields we need; tokens are never returned.
fn remote_info(bin: &str, name: &str) -> Option<RemoteInfo> {
    let out = rclone_command(bin).args(["config", "dump"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let r = v.get(name)?;
    let s = |k: &str| r.get(k).and_then(|x| x.as_str()).map(str::to_string);
    Some(RemoteInfo {
        name: name.to_string(),
        kind: s("type").unwrap_or_default(),
        scope: s("scope"),
        custom_client: s("client_id").map(|c| !c.trim().is_empty()).unwrap_or(false),
    })
}

pub fn status(configured_bin: Option<&str>, remote: &str) -> RcloneStatus {
    let bin = resolve_rclone(configured_bin);
    let version = version_of(&bin);
    let remote = version.as_ref().and_then(|_| remote_info(&bin, remote));
    RcloneStatus {
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        platform: platform(),
        found: version.is_some(),
        managed: managed_rclone_path().to_string_lossy() == bin,
        bin,
        version,
        remote,
    }
}

fn get_string(url: &str) -> Result<String> {
    ureq::get(url)
        .timeout(Duration::from_secs(60))
        .call()
        .with_context(|| tr!("err.download_url", url = url).to_string())?
        .into_string()
        .context(tr!("err.bad_response").to_string())
}

/// Download the latest rclone from downloads.rclone.org, verify its SHA-256 and unpack it into the app folder.
pub fn install() -> Result<String> {
    let plat = platform().context(tr!("err.rclone_no_build").to_string())?;
    let ver_line = get_string(&format!("{DOWNLOADS}/version.txt"))?;
    let ver = ver_line.trim().strip_prefix("rclone ").unwrap_or(ver_line.trim()).to_string();
    if !ver.starts_with('v') || !ver[1..].chars().all(|c| c.is_ascii_digit() || c == '.') {
        bail!("{}", tr!("err.rclone_bad_version", version = ver_line.trim()));
    }
    let file = format!("rclone-{ver}-{plat}.zip");
    let sums = get_string(&format!("{DOWNLOADS}/{ver}/SHA256SUMS"))?;
    let expected = sums
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            Some((it.next()?, it.next()?))
        })
        .find(|(_, name)| *name == file)
        .map(|(h, _)| h.to_lowercase())
        .with_context(|| tr!("err.rclone_no_checksum", file = file).to_string())?;

    let resp = ureq::get(&format!("{DOWNLOADS}/{ver}/{file}"))
        .timeout(Duration::from_secs(300))
        .call()
        .context(tr!("err.rclone_download").to_string())?;
    let mut buf = Vec::new();
    resp.into_reader().take(200 * 1024 * 1024).read_to_end(&mut buf)?;
    let got = hex::encode(Sha256::digest(&buf));
    if got != expected {
        bail!("{}", tr!("err.rclone_checksum"));
    }

    let exe_name = if cfg!(windows) { "rclone.exe" } else { "rclone" };
    let mut za = zip::ZipArchive::new(std::io::Cursor::new(buf)).context(tr!("err.rclone_bad_zip").to_string())?;
    let idx = (0..za.len())
        .find(|&i| za.by_index(i).map(|e| e.name().rsplit('/').next() == Some(exe_name)).unwrap_or(false))
        .context(tr!("err.rclone_bad_zip").to_string())?;
    let dest = managed_rclone_path();
    fs::create_dir_all(dest.parent().unwrap())?;
    let tmp = dest.with_extension("download");
    {
        let mut e = za.by_index(idx)?;
        let mut out = fs::File::create(&tmp)?;
        std::io::copy(&mut e, &mut out)?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
    }
    fs::rename(&tmp, &dest)?;
    let v = version_of(&dest.to_string_lossy()).context(tr!("err.rclone_not_runnable").to_string())?;
    Ok(v)
}

fn valid_client_id(s: &str) -> bool {
    s.ends_with(".apps.googleusercontent.com")
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'))
}

/// Create (or recreate) the Google Drive remote with the user's own client_id. Opens the browser for Google sign-in.
pub fn connect(configured_bin: Option<&str>, name: &str, client_id: &str, client_secret: &str) -> Result<()> {
    let client_id = client_id.trim();
    let client_secret = client_secret.trim();
    if !valid_client_id(client_id) {
        bail!("{}", tr!("err.client_id_format"));
    }
    if client_secret.is_empty() || client_secret.chars().any(|c| c.is_whitespace()) {
        bail!("{}", tr!("err.client_secret_format"));
    }
    let bin = resolve_rclone(configured_bin);
    version_of(&bin).context(tr!("err.rclone_missing").to_string())?;

    if remote_info(&bin, name).is_some() {
        let out = rclone_command(&bin).args(["config", "delete", name]).output()?;
        if !out.status.success() {
            bail!("{}", tr!("err.rclone_delete_remote", detail = String::from_utf8_lossy(&out.stderr).trim()));
        }
    }

    let mut child = rclone_command(&bin)
        .args([
            "config",
            "create",
            name,
            "drive",
            "scope=drive.file",
            &format!("client_id={client_id}"),
            &format!("client_secret={client_secret}"),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| tr!("err.rclone_run", bin = bin).to_string())?;

    // Wait for the user to sign in in the browser, at most 5 minutes
    let start = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if start.elapsed() > Duration::from_secs(300) {
            let _ = child.kill();
            let _ = rclone_command(&bin).args(["config", "delete", name]).output();
            bail!("{}", tr!("err.signin_timeout"));
        }
        std::thread::sleep(Duration::from_millis(400));
    };
    if !status.success() {
        let mut err = String::new();
        if let Some(mut e) = child.stderr.take() {
            let _ = e.read_to_string(&mut err);
        }
        let last = err.lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("").to_string();
        bail!("{}", tr!("err.signin_failed", detail = last));
    }

    let out = rclone_command(&bin).args(["mkdir", &format!("{name}:Checkpoint")]).output()?;
    if !out.status.success() {
        bail!("{}", tr!("err.drive_folder", detail = String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_id_validation() {
        assert!(valid_client_id("123-abc.apps.googleusercontent.com"));
        assert!(!valid_client_id("123 abc.apps.googleusercontent.com"));
        assert!(!valid_client_id("x;rm -rf.apps.googleusercontent.com"));
        assert!(!valid_client_id("abc"));
    }

    /// `cargo test install_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn install_real() {
        let v = install().expect("install rclone");
        println!("installed: {v} -> {}", managed_rclone_path().display());
        let st = status(None, "gdrive");
        println!("status: found={} managed={} version={:?} remote={:?}", st.found, st.managed, st.version, st.remote);
        assert!(st.managed);
    }

    #[test]
    fn platform_known() {
        assert!(platform().is_some());
    }
}
