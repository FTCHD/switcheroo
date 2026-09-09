//! Background tray. `switcheroo tray` spawns `tray --foreground` as a detached child (own
//! session on unix, no console on Windows), waits for it to publish `server.json`, and returns.
//! `--stop` terminates that child; the foreground process removes `server.json` on the way out.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::core::Core;
use crate::core::fsutil::remove_opt;
use crate::core::proc::pid_alive;
use crate::server::{ServerInfo, running_server};

pub enum Started {
    AlreadyRunning(ServerInfo),
    Spawned(ServerInfo),
}

pub fn spawn_detached(core: &Core, vault_override: Option<&str>) -> Result<Started> {
    if let Some(info) = running_server(core)
        && pid_alive(info.pid)
    {
        return Ok(Started::AlreadyRunning(info));
    }
    let exe = std::env::current_exe().context("locating the switcheroo binary")?;
    let log_path = core.dirs.data.join("tray.log");
    let log = std::fs::OpenOptions::new().create(true).append(true).open(&log_path)?;
    let err = log.try_clone()?;
    let mut cmd = Command::new(exe);
    cmd.arg("tray")
        .arg("--foreground")
        .arg("--data-dir")
        .arg(&core.dirs.data)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(err));
    if let Some(v) = vault_override {
        cmd.env("SWITCHEROO_VAULT", v);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // A new session: the terminal's hangup never reaches the tray.
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    let mut child = cmd.spawn().context("starting the tray")?;
    let pid = child.id();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if let Some(status) = child.try_wait()? {
            bail!("the tray exited right away ({status}); see {}", log_path.display());
        }
        if let Some(info) = running_server(core)
            && info.pid == pid
        {
            return Ok(Started::Spawned(info));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    bail!("the tray did not come up within 10 seconds; see {}", log_path.display())
}

/// Terminate the background tray. `Ok(None)` when none was running.
pub fn stop(core: &Core) -> Result<Option<u32>> {
    let Some(info) = running_server(core).filter(|i| pid_alive(i.pid)) else {
        let _ = remove_opt(&core.dirs.server_file());
        return Ok(None);
    };
    terminate(info.pid)?;
    let start = Instant::now();
    while pid_alive(info.pid) && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(100));
    }
    if pid_alive(info.pid) {
        bail!("the tray (pid {}) did not exit", info.pid);
    }
    let _ = remove_opt(&core.dirs.server_file());
    Ok(Some(info.pid))
}

#[cfg(unix)]
fn terminate(pid: u32) -> Result<()> {
    if unsafe { libc::kill(pid as i32, libc::SIGTERM) } != 0 {
        return Err(std::io::Error::last_os_error()).context("sending SIGTERM");
    }
    Ok(())
}

#[cfg(windows)]
fn terminate(pid: u32) -> Result<()> {
    let out = Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]).output().context("running taskkill")?;
    if !out.status.success() {
        bail!("taskkill failed: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}
