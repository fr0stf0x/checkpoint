//! One long-running `rclone rcd` per Checkpoint process, instead of starting rclone for every
//! storage call. Starting rclone costs about 3 s each time (process start, token refresh, Drive
//! folder lookups); the daemon keeps the Drive connection and its folder cache warm.
//!
//! - Listens on 127.0.0.1 only, on a free port, with a random user/password (passed through the
//!   environment, not the command line), so other programs on the PC cannot use it.
//! - Calls are plain HTTP POSTs to rclone's remote-control API (`operations/copyfile`,
//!   `operations/deletefile`, `operations/purge`).
//! - Stopped when the app exits. If the app crashes, the next start kills the orphan: each process
//!   records its daemon in `<data>/rcd/<app pid>.json`.
//! - If the daemon cannot start, callers fall back to running rclone per call (storage.rs).

use crate::config::data_dir;
use crate::storage::rclone_command;
use anyhow::{anyhow, bail, Context, Result};
use log::{info, warn};
use rand::Rng;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A running daemon's address and credentials.
pub struct Rc {
    base: String,
    auth: String,
    agent: ureq::Agent,
}

/// Why a call failed: the daemon is unreachable (use the CLI instead), or rclone reported an error.
#[derive(Debug)]
pub enum RcError {
    Transport(String),
    Api(String),
}

impl RcError {
    pub fn is_not_found(&self) -> bool {
        match self {
            RcError::Api(m) => {
                let m = m.to_lowercase();
                m.contains("not found") || m.contains("doesn't exist") || m.contains("does not exist")
            }
            RcError::Transport(_) => false,
        }
    }
}

impl Rc {
    pub fn call(&self, path: &str, body: Value) -> Result<Value, RcError> {
        let r = self
            .agent
            .post(&format!("{}/{path}", self.base))
            .set("Authorization", &self.auth)
            .set("Content-Type", "application/json")
            .send_string(&body.to_string());
        let parse = |resp: ureq::Response| -> Value { resp.into_string().ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null) };
        match r {
            Ok(resp) => Ok(parse(resp)),
            Err(ureq::Error::Status(code, resp)) => {
                let v = parse(resp);
                let msg = v.get("error").and_then(|e| e.as_str()).map(str::to_string).unwrap_or_else(|| format!("HTTP {code}"));
                Err(RcError::Api(msg))
            }
            Err(e) => Err(RcError::Transport(e.to_string())),
        }
    }

    /// Copy one file between two rclone file systems (a local folder path or `remote:path`).
    pub fn copy_file(&self, src_fs: &str, src: &str, dst_fs: &str, dst: &str) -> Result<(), RcError> {
        self.call("operations/copyfile", json!({ "srcFs": src_fs, "srcRemote": src, "dstFs": dst_fs, "dstRemote": dst }))
            .map(|_| ())
    }

    /// `copy_file` as a background job, polled every 250 ms so `on(bytes, total)` can show the
    /// transfer (total is 0 until rclone knows it).
    pub fn copy_file_watch(&self, src_fs: &str, src: &str, dst_fs: &str, dst: &str, on: &dyn Fn(u64, u64)) -> Result<(), RcError> {
        let r = self.call(
            "operations/copyfile",
            json!({ "srcFs": src_fs, "srcRemote": src, "dstFs": dst_fs, "dstRemote": dst, "_async": true }),
        )?;
        let Some(id) = r.get("jobid").and_then(Value::as_u64) else {
            return Err(RcError::Api("rclone did not return a job id".into()));
        };
        loop {
            std::thread::sleep(Duration::from_millis(250));
            if let Ok(s) = self.call("core/stats", json!({ "group": format!("job/{id}") })) {
                let item = s.get("transferring").and_then(Value::as_array).and_then(|a| a.first());
                let num = |v: Option<&Value>, k: &str| v.and_then(|v| v.get(k)).and_then(Value::as_u64).unwrap_or(0);
                let (done, total) = match item {
                    Some(_) => (num(item, "bytes"), num(item, "size")),
                    None => (num(Some(&s), "bytes"), num(Some(&s), "totalBytes")),
                };
                on(done, total);
            }
            let st = self.call("job/status", json!({ "jobid": id }))?;
            if st.get("finished").and_then(Value::as_bool) == Some(true) {
                if st.get("success").and_then(Value::as_bool) == Some(true) {
                    return Ok(());
                }
                let e = st.get("error").and_then(Value::as_str).filter(|e| !e.is_empty()).unwrap_or("copy failed");
                return Err(RcError::Api(e.to_string()));
            }
        }
    }

    pub fn delete_file(&self, fs: &str, remote: &str) -> Result<(), RcError> {
        self.call("operations/deletefile", json!({ "fs": fs, "remote": remote })).map(|_| ())
    }

    pub fn purge(&self, fs: &str, remote: &str) -> Result<(), RcError> {
        self.call("operations/purge", json!({ "fs": fs, "remote": remote })).map(|_| ())
    }
}

struct Daemon {
    child: Child,
    bin: String,
    rc: Arc<Rc>,
}

static DAEMON: Mutex<Option<Daemon>> = Mutex::new(None);
/// Set after a failed start, so a broken setup does not cost a start attempt on every call.
static FAILED: AtomicBool = AtomicBool::new(false);

/// The daemon for `bin`, started on first use. None means: run rclone per call instead.
pub fn client(bin: &str) -> Option<Arc<Rc>> {
    if std::env::var_os("CHECKPOINT_NO_RCD").is_some() {
        return None;
    }
    let mut g = DAEMON.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(d) = g.as_mut() {
        let alive = matches!(d.child.try_wait(), Ok(None));
        if alive && d.bin == bin {
            return Some(d.rc.clone());
        }
        if !alive {
            warn!("rclone rcd stopped unexpectedly, starting it again");
        }
        stop(g.take());
    }
    if FAILED.load(Ordering::Relaxed) {
        return None;
    }
    match start(bin) {
        Ok(d) => {
            let rc = d.rc.clone();
            *g = Some(d);
            Some(rc)
        }
        Err(e) => {
            FAILED.store(true, Ordering::Relaxed);
            warn!("rclone rcd could not start, running rclone per call instead: {e:#}");
            None
        }
    }
}

/// Forget the daemon after it stopped answering (the caller retries with the CLI).
pub fn reset() {
    let mut g = DAEMON.lock().unwrap_or_else(|e| e.into_inner());
    stop(g.take());
}

/// Stop the daemon: on app exit, and before the rclone config changes (new Google sign-in).
pub fn shutdown() {
    reset();
    FAILED.store(false, Ordering::Relaxed);
}

fn stop(d: Option<Daemon>) {
    if let Some(mut d) = d {
        let _ = d.child.kill();
        let _ = d.child.wait();
        let _ = fs::remove_file(record_path(std::process::id()));
        info!("rclone rcd stopped");
    }
}

fn record_dir() -> PathBuf {
    data_dir().join("rcd")
}

fn record_path(app_pid: u32) -> PathBuf {
    record_dir().join(format!("{app_pid}.json"))
}

fn start(bin: &str) -> Result<Daemon> {
    kill_orphans();
    let port = std::net::TcpListener::bind("127.0.0.1:0")?.local_addr()?.port();
    let pass: String = rand::thread_rng().sample_iter(&rand::distributions::Alphanumeric).take(32).map(char::from).collect();
    let user = "checkpoint";
    let t = Instant::now();
    let mut cmd = rclone_command(bin);
    cmd.args(["rcd", "--rc-addr", &format!("127.0.0.1:{port}")])
        .env("RCLONE_RC_USER", user)
        .env("RCLONE_RC_PASS", &pass)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    die_with_parent::before_spawn(&mut cmd);
    let child = cmd.spawn().context("spawn rclone rcd")?;
    die_with_parent::after_spawn(&child);
    let mut d = Daemon {
        child,
        bin: bin.to_string(),
        rc: Arc::new(Rc {
            base: format!("http://127.0.0.1:{port}"),
            auth: format!("Basic {}", base64(format!("{user}:{pass}").as_bytes())),
            agent: ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(5)).build(),
        }),
    };
    let _ = fs::create_dir_all(record_dir());
    let _ = fs::write(record_path(std::process::id()), json!({ "rclone_pid": d.child.id() }).to_string());
    // Wait until it answers
    loop {
        if let Ok(Some(st)) = d.child.try_wait() {
            let _ = fs::remove_file(record_path(std::process::id()));
            bail!("rclone rcd exited with {st}");
        }
        match d.rc.call("rc/noop", json!({})) {
            Ok(_) => break,
            Err(RcError::Api(e)) => {
                stop(Some(d));
                bail!("rclone rcd refused the call: {e}");
            }
            Err(RcError::Transport(_)) if t.elapsed() < Duration::from_secs(15) => std::thread::sleep(Duration::from_millis(100)),
            Err(RcError::Transport(e)) => {
                stop(Some(d));
                return Err(anyhow!("rclone rcd did not answer within 15 s: {e}"));
            }
        }
    }
    info!("rclone rcd started on 127.0.0.1:{port} in {} ms", t.elapsed().as_millis());
    Ok(d)
}

/// Make the daemon end when this process ends, even when it is killed. Steam waits for every
/// process a game's launch options started, so a daemon left behind by a killed launch wrapper
/// kept the game "running" (Stopping…) in Steam. `kill_orphans` stays as the fallback.
mod die_with_parent {
    use std::process::{Child, Command};

    /// Windows: a job object that kills its processes when its last handle closes, i.e. when
    /// this process ends. The handle is not inheritable, so rclone does not keep it open.
    #[cfg(windows)]
    pub fn before_spawn(_: &mut Command) {}

    #[cfg(windows)]
    pub fn after_spawn(child: &Child) {
        use std::os::windows::io::AsRawHandle;
        use std::sync::OnceLock;
        use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        static JOB: OnceLock<usize> = OnceLock::new();
        let job = *JOB.get_or_init(|| unsafe {
            let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if h.is_null() {
                return 0;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let size = std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
            if SetInformationJobObject(h, JobObjectExtendedLimitInformation, &info as *const _ as *const _, size) == 0 {
                CloseHandle(h);
                return 0;
            }
            h as usize
        });
        if job == 0 || unsafe { AssignProcessToJobObject(job as HANDLE, child.as_raw_handle() as HANDLE) } == 0 {
            log::warn!("rclone rcd is not tied to this process: {}", std::io::Error::last_os_error());
        }
    }

    /// Linux: the kernel sends SIGTERM to rclone when its parent dies. PR_SET_PDEATHSIG fires
    /// when the spawning *thread* ends, so it is only set from the main thread (the launch
    /// wrapper); the app spawns from worker threads that come and go, and relies on
    /// `kill_orphans` instead.
    #[cfg(target_os = "linux")]
    pub fn before_spawn(cmd: &mut Command) {
        use std::os::unix::process::CommandExt;
        if unsafe { libc::gettid() != libc::getpid() } {
            return;
        }
        let parent = std::process::id() as libc::pid_t;
        unsafe {
            cmd.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                // The parent may have died before prctl took effect
                if libc::getppid() != parent {
                    libc::_exit(0);
                }
                Ok(())
            });
        }
    }

    #[cfg(target_os = "linux")]
    pub fn after_spawn(_: &Child) {}

    #[cfg(not(any(windows, target_os = "linux")))]
    pub fn before_spawn(_: &mut Command) {}

    #[cfg(not(any(windows, target_os = "linux")))]
    pub fn after_spawn(_: &Child) {}
}

/// Kill daemons left behind by Checkpoint processes that are gone (crash, killed).
fn kill_orphans() {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
    let Ok(rd) = fs::read_dir(record_dir()) else { return };
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    for e in rd.flatten() {
        let path = e.path();
        let Some(app_pid) = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<u32>().ok()) else { continue };
        if app_pid == std::process::id() || sys.process(Pid::from_u32(app_pid)).is_some() {
            continue;
        }
        let rclone_pid = fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.get("rclone_pid").and_then(Value::as_u64));
        if let Some(p) = rclone_pid.and_then(|p| sys.process(Pid::from_u32(p as u32))) {
            if p.name().to_string_lossy().to_lowercase().contains("rclone") {
                p.kill();
                info!("Stopped an rclone rcd left behind by an earlier Checkpoint run");
            }
        }
        let _ = fs::remove_file(&path);
    }
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// rclone file system string and file name for a local file: (`<folder>`, `<name>`).
pub fn local_parts(p: &Path) -> Option<(String, String)> {
    Some((p.parent()?.to_string_lossy().to_string(), p.file_name()?.to_string_lossy().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"checkpoint:pass"), "Y2hlY2twb2ludDpwYXNz");
    }

    #[test]
    fn not_found_detection() {
        assert!(RcError::Api("object not found".into()).is_not_found());
        assert!(RcError::Api("directory not found".into()).is_not_found());
        assert!(!RcError::Api("permission denied".into()).is_not_found());
        assert!(!RcError::Transport("connection refused".into()).is_not_found());
    }
}
