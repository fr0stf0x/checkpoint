//! Nơi lưu backup: thư mục thường (Drive for Desktop / rclone mount) hoặc rclone remote.

use crate::config::Backend;
use anyhow::{bail, Context, Result};
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
}

pub fn open(backend: &Backend) -> Result<Box<dyn Storage>> {
    match backend {
        Backend::None => bail!("Chưa chọn nơi lưu backup. Vào Cài đặt để kết nối Google Drive."),
        Backend::Folder { path } => {
            let root = PathBuf::from(path);
            if !root.exists() {
                fs::create_dir_all(&root).with_context(|| format!("Không tạo được thư mục {path}"))?;
            }
            Ok(Box::new(FolderStorage { root }))
        }
        Backend::Rclone { remote, bin } => Ok(Box::new(RcloneStorage {
            remote: remote.trim_end_matches('/').to_string(),
            bin: resolve_rclone(bin.as_deref()),
        })),
    }
}

/// Tìm rclone: đường dẫn trong cấu hình → PATH → thư mục cài của winget (bản mới nhất).
pub fn resolve_rclone(configured: Option<&str>) -> String {
    if let Some(b) = configured.map(str::trim).filter(|b| !b.is_empty()) {
        return b.to_string();
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
        fs::copy(local, &p).with_context(|| format!("Không copy được lên {}", p.display()))?;
        Ok(())
    }
    fn download(&self, rel: &str, local: &Path) -> Result<()> {
        fs::copy(self.full(rel), local).context("Không tải được file backup")?;
        Ok(())
    }
    fn delete(&self, rel: &str) -> Result<()> {
        let p = self.full(rel);
        if p.exists() {
            fs::remove_file(p)?;
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

impl RcloneStorage {
    fn target(&self, rel: &str) -> String {
        format!("{}/{}", self.remote, rel)
    }
    fn run(&self, args: &[&str]) -> Result<std::process::Output> {
        rclone_command(&self.bin)
            .args(args)
            .output()
            .with_context(|| format!("Không chạy được rclone ({}). Đã cài rclone chưa?", self.bin))
    }
    fn check(out: std::process::Output, what: &str) -> Result<()> {
        if !out.status.success() {
            bail!("rclone {what} lỗi: {}", String::from_utf8_lossy(&out.stderr).trim());
        }
        Ok(())
    }
}

impl Storage for RcloneStorage {
    fn read(&self, rel: &str) -> Result<Option<Vec<u8>>> {
        let out = self.run(&["cat", &self.target(rel)])?;
        if out.status.success() {
            return Ok((!out.stdout.is_empty()).then_some(out.stdout));
        }
        let err = String::from_utf8_lossy(&out.stderr).to_lowercase();
        if err.contains("not found") || err.contains("doesn't exist") || err.contains("does not exist") {
            return Ok(None);
        }
        bail!("rclone cat lỗi: {}", err.trim())
    }
    fn write(&self, rel: &str, data: &[u8]) -> Result<()> {
        let mut child = rclone_command(&self.bin)
            .args(["rcat", &self.target(rel)])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Không chạy được rclone")?;
        child.stdin.take().context("rclone stdin")?.write_all(data)?;
        Self::check(child.wait_with_output()?, "rcat")
    }
    fn upload(&self, local: &Path, rel: &str) -> Result<()> {
        let out = self.run(&["copyto", &local.to_string_lossy(), &self.target(rel)])?;
        Self::check(out, "upload")
    }
    fn download(&self, rel: &str, local: &Path) -> Result<()> {
        let out = self.run(&["copyto", &self.target(rel), &local.to_string_lossy()])?;
        Self::check(out, "download")
    }
    fn delete(&self, rel: &str) -> Result<()> {
        let out = self.run(&["deletefile", &self.target(rel)])?;
        Self::check(out, "delete")
    }
}
