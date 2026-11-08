//! Backup storage: a plain folder (Drive for Desktop / rclone mount) or an rclone remote.

use crate::config::Backend;
use anyhow::{bail, Context, Result};
use crate::tr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub trait Storage: Send {
    fn read(&self, rel: &str) -> Result<Option<Vec<u8>>>;
    fn write(&self, rel: &str, data: &[u8]) -> Result<()>;
    fn upload(&self, local: &Path, rel: &str) -> Result<()>;
    fn download(&self, rel: &str, local: &Path) -> Result<()>;
    fn delete(&self, rel: &str) -> Result<()>;
    /// Remove a folder and everything in it. A missing folder is not an error.
    fn delete_dir(&self, rel: &str) -> Result<()>;
}

pub fn open(backend: &Backend) -> Result<Box<dyn Storage>> {
    match backend {
        Backend::None => bail!("{}", tr!("err.no_backend")),
        Backend::Folder { path } => {
            let root = PathBuf::from(path);
            if !root.exists() {
                fs::create_dir_all(&root).with_context(|| tr!("err.create_folder", path = path).to_string())?;
            }
            Ok(Box::new(FolderStorage { root }))
        }
        Backend::Rclone { remote, bin } => Ok(Box::new(RcloneStorage {
            remote: remote.trim_end_matches('/').to_string(),
            bin: resolve_rclone(bin.as_deref()),
        })),
    }
}

/// Where the app installs its own rclone (no admin rights needed).
pub fn managed_rclone_path() -> PathBuf {
    crate::config::data_dir().join("bin").join(if cfg!(windows) { "rclone.exe" } else { "rclone" })
}

/// Find rclone: configured path → app-managed copy → PATH → winget install folder (newest).
pub fn resolve_rclone(configured: Option<&str>) -> String {
    if let Some(b) = configured.map(str::trim).filter(|b| !b.is_empty()) {
        return b.to_string();
    }
    let managed = managed_rclone_path();
    if managed.exists() {
        return managed.to_string_lossy().to_string();
    }
    let in_path = rclone_command("rclone").arg("version").output().map(|o| o.status.success()).unwrap_or(false);
    if in_path {
        return "rclone".into();
    }
    #[cfg(windows)]
    if let Some(local) = dirs::data_local_dir() {
        let pattern = local.join("Microsoft/WinGet/Packages/Rclone.Rclone_*/rclone-*/rclone.exe");
        let mut found: Vec<PathBuf> = glob::glob(&pattern.to_string_lossy()).into_iter().flatten().flatten().collect();
        found.sort();
        if let Some(p) = found.pop() {
            return p.to_string_lossy().to_string();
        }
    }
    "rclone".into()
}

pub struct FolderStorage {
    root: PathBuf,
}

impl FolderStorage {
    fn full(&self, rel: &str) -> PathBuf {
        let mut p = self.root.clone();
        for part in rel.split('/') {
            p.push(part);
        }
        p
    }
}

impl Storage for FolderStorage {
    fn read(&self, rel: &str) -> Result<Option<Vec<u8>>> {
        let p = self.full(rel);
        if !p.exists() {
            return Ok(None);
        }
        Ok(Some(fs::read(p)?))
    }
    fn write(&self, rel: &str, data: &[u8]) -> Result<()> {
        let p = self.full(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = p.with_extension("tmp-checkpoint");
        fs::write(&tmp, data)?;
        fs::rename(&tmp, &p)?;
        Ok(())
    }
    fn upload(&self, local: &Path, rel: &str) -> Result<()> {
        let p = self.full(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(local, &p).with_context(|| tr!("err.copy_to", path = p.display()).to_string())?;
        Ok(())
    }
    fn download(&self, rel: &str, local: &Path) -> Result<()> {
        fs::copy(self.full(rel), local).context(tr!("err.download_backup").to_string())?;
        Ok(())
    }
    fn delete(&self, rel: &str) -> Result<()> {
        let p = self.full(rel);
        if p.exists() {
            fs::remove_file(p)?;
        }
        Ok(())
    }
    fn delete_dir(&self, rel: &str) -> Result<()> {
        let p = self.full(rel);
        if p.exists() {
            fs::remove_dir_all(&p).with_context(|| tr!("err.delete_path", path = p.display()).to_string())?;
        }
        Ok(())
    }
}

pub struct RcloneStorage {
    remote: String,
    bin: String,
}

pub fn rclone_command(bin: &str) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(bin);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c
}

/// rclone failed because the file or folder is not there (fine when deleting).
fn is_not_found(out: &std::process::Output) -> bool {
    let err = String::from_utf8_lossy(&out.stderr).to_lowercase();
    !out.status.success() && (err.contains("not found") || err.contains("doesn't exist") || err.contains("does not exist"))
}

impl RcloneStorage {
    fn target(&self, rel: &str) -> String {
        format!("{}/{}", self.remote, rel)
    }
    fn run(&self, args: &[&str]) -> Result<std::process::Output> {
        rclone_command(&self.bin)
            .args(args)
            .output()
            .with_context(|| tr!("err.rclone_run", bin = self.bin).to_string())
    }
    fn check(out: std::process::Output, what: &str) -> Result<()> {
        if !out.status.success() {
            bail!("{}", tr!("err.rclone_failed", what = what, detail = String::from_utf8_lossy(&out.stderr).trim()));
        }
        Ok(())
    }

    /// Run `f` against the rclone daemon (rcd.rs). `Ok(None)` means: no daemon, or it stopped
    /// answering; the caller then runs rclone per call as before.
    fn via_rc<T>(&self, what: &str, f: impl FnOnce(&crate::rcd::Rc) -> Result<T, crate::rcd::RcError>) -> Result<Option<T>> {
        let Some(rc) = crate::rcd::client(&self.bin) else { return Ok(None) };
        match f(&rc) {
            Ok(v) => Ok(Some(v)),
            Err(crate::rcd::RcError::Transport(e)) => {
                log::warn!("rclone rcd did not answer ({what}: {e}), running rclone directly");
                crate::rcd::reset();
                Ok(None)
            }
            Err(crate::rcd::RcError::Api(e)) => bail!("{}", tr!("err.rclone_failed", what = what, detail = e)),
        }
    }

    /// A fresh temporary folder for one transfer through the daemon.
    fn tmp_dir() -> Result<PathBuf> {
        use rand::Rng;
        let n: u64 = rand::thread_rng().gen();
        let d = crate::config::cache_dir().join("tmp").join(format!("rc-{n:016x}"));
        fs::create_dir_all(&d)?;
        Ok(d)
    }

    // ---------- one rclone process per call (fallback) ----------

    fn cli_read(&self, rel: &str) -> Result<Option<Vec<u8>>> {
        let out = self.run(&["cat", &self.target(rel)])?;
        if out.status.success() {
            return Ok((!out.stdout.is_empty()).then_some(out.stdout));
        }
        if is_not_found(&out) {
            return Ok(None);
        }
        Self::check(out, "cat").map(|_| None)
    }
    fn cli_write(&self, rel: &str, data: &[u8]) -> Result<()> {
        let mut child = rclone_command(&self.bin)
            .args(["rcat", &self.target(rel)])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| tr!("err.rclone_run", bin = self.bin).to_string())?;
        child.stdin.take().context("rclone stdin")?.write_all(data)?;
        Self::check(child.wait_with_output()?, "rcat")
    }
    fn cli_copyto(&self, from: &str, to: &str, what: &str) -> Result<()> {
        let out = self.run(&["copyto", from, to])?;
        Self::check(out, what)
    }
    fn cli_delete(&self, cmd: &str, rel: &str) -> Result<()> {
        let out = self.run(&[cmd, &self.target(rel)])?;
        if is_not_found(&out) {
            return Ok(());
        }
        Self::check(out, cmd)
    }
}

impl Storage for RcloneStorage {
    fn read(&self, rel: &str) -> Result<Option<Vec<u8>>> {
        let dir = Self::tmp_dir()?;
        let r = self.via_rc("cat", |rc| match rc.copy_file(&self.remote, rel, &dir.to_string_lossy(), "f") {
            Ok(()) => Ok(true),
            Err(e) if e.is_not_found() => Ok(false),
            Err(e) => Err(e),
        });
        let data = match r {
            Ok(Some(true)) => fs::read(dir.join("f")).map(|d| (!d.is_empty()).then_some(d)).map_err(Into::into),
            Ok(Some(false)) => Ok(None),
            Ok(None) => self.cli_read(rel),
            Err(e) => Err(e),
        };
        let _ = fs::remove_dir_all(&dir);
        data
    }
    fn write(&self, rel: &str, data: &[u8]) -> Result<()> {
        let dir = Self::tmp_dir()?;
        let r = fs::write(dir.join("f"), data)
            .map_err(anyhow::Error::from)
            .and_then(|_| self.via_rc("rcat", |rc| rc.copy_file(&dir.to_string_lossy(), "f", &self.remote, rel)));
        let _ = fs::remove_dir_all(&dir);
        match r? {
            Some(()) => Ok(()),
            None => self.cli_write(rel, data),
        }
    }
    fn upload(&self, local: &Path, rel: &str) -> Result<()> {
        let (fs_, name) = crate::rcd::local_parts(local).context("upload path")?;
        match self.via_rc("upload", |rc| rc.copy_file(&fs_, &name, &self.remote, rel))? {
            Some(()) => Ok(()),
            None => self.cli_copyto(&local.to_string_lossy(), &self.target(rel), "upload"),
        }
    }
    fn download(&self, rel: &str, local: &Path) -> Result<()> {
        let (fs_, name) = crate::rcd::local_parts(local).context("download path")?;
        match self.via_rc("download", |rc| rc.copy_file(&self.remote, rel, &fs_, &name))? {
            Some(()) => Ok(()),
            None => self.cli_copyto(&self.target(rel), &local.to_string_lossy(), "download"),
        }
    }
    fn delete(&self, rel: &str) -> Result<()> {
        let r = self.via_rc("delete", |rc| match rc.delete_file(&self.remote, rel) {
            Err(e) if e.is_not_found() => Ok(()),
            other => other,
        })?;
        match r {
            Some(()) => Ok(()),
            None => self.cli_delete("deletefile", rel),
        }
    }
    fn delete_dir(&self, rel: &str) -> Result<()> {
        let r = self.via_rc("purge", |rc| match rc.purge(&self.remote, rel) {
            Err(e) if e.is_not_found() => Ok(()),
            other => other,
        })?;
        match r {
            Some(()) => Ok(()),
            None => self.cli_delete("purge", rel),
        }
    }
}
