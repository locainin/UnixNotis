//! Child process management for UI components.
//!
//! Keeps spawn/termination logic for popups and center processes in one place.

use std::env;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use tokio::time::sleep;
use tracing::warn;

use super::Args;

#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;

pub(super) fn start_popups_process(args: &Args) -> Result<Option<Child>> {
    let Some(mut command) = build_popups_command(args)? else {
        return Ok(None);
    };
    // Spawn the popup UI as a child process so resource usage is attributed correctly.
    let child = command.spawn().map_err(|err| {
        anyhow!(
            "failed to start unixnotis-popups ({}); build it or install it on PATH",
            err
        )
    })?;
    Ok(Some(child))
}

pub(super) async fn stop_popups_process(child: &mut Child) {
    terminate_child(child, "unixnotis-popups").await;
}

pub(super) fn start_center_process(args: &Args) -> Result<Option<Child>> {
    let Some(mut command) = build_center_command(args)? else {
        return Ok(None);
    };
    // Spawn the panel UI as a child process so resource usage is attributed correctly.
    match command.spawn() {
        Ok(child) => Ok(Some(child)),
        Err(err) => {
            warn!(
                ?err,
                "failed to start unixnotis-center; build it or install it on PATH"
            );
            Ok(None)
        }
    }
}

pub(super) async fn stop_center_process(child: &mut Child) {
    terminate_child(child, "unixnotis-center").await;
}

async fn terminate_child(child: &mut Child, label: &str) {
    let pid = child.id();
    #[cfg(unix)]
    unsafe {
        let pid = match i32::try_from(pid) {
            Ok(pid) => pid,
            Err(_) => {
                warn!(label, pid, "pid exceeds i32 range; skipping SIGTERM");
                return;
            }
        };
        libc::kill(pid, libc::SIGTERM);
    }
    let start = Instant::now();
    let timeout = Duration::from_millis(600);
    while start.elapsed() < timeout {
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        // Async sleep avoids blocking the runtime during shutdown.
        sleep(Duration::from_millis(50)).await;
    }
    warn!(label, pid, "force killing unresponsive child process");
    let _ = child.kill();
    let _ = child.wait();
}

fn build_popups_command(args: &Args) -> Result<Option<Command>> {
    let mut command = if let Some(path) = resolve_popups_path() {
        Command::new(path)
    } else {
        Command::new("unixnotis-popups")
    };

    apply_parent_death_signal(&mut command);

    if let Some(config) = args.config.as_ref() {
        command.arg("--config").arg(config);
    }

    Ok(Some(command))
}

fn resolve_popups_path() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let dir = exe.parent()?;
    let candidate = dir.join("unixnotis-popups");
    if candidate.is_file() {
        return Some(candidate);
    }
    let candidate = dir.join("unixnotis-popups.exe");
    if candidate.is_file() {
        return Some(candidate);
    }
    None
}

fn build_center_command(args: &Args) -> Result<Option<Command>> {
    let mut command = if let Some(path) = resolve_center_path() {
        Command::new(path)
    } else {
        Command::new("unixnotis-center")
    };

    apply_parent_death_signal(&mut command);

    if let Some(config) = args.config.as_ref() {
        command.arg("--config").arg(config);
    }

    Ok(Some(command))
}

#[cfg(target_os = "linux")]
fn apply_parent_death_signal(command: &mut Command) {
    // Ensure UI subprocesses exit if the daemon dies unexpectedly.
    unsafe {
        command.pre_exec(|| {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(not(target_os = "linux"))]
fn apply_parent_death_signal(_command: &mut Command) {}

fn resolve_center_path() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let dir = exe.parent()?;
    let candidate = dir.join("unixnotis-center");
    if candidate.is_file() {
        return Some(candidate);
    }
    let candidate = dir.join("unixnotis-center.exe");
    if candidate.is_file() {
        return Some(candidate);
    }
    None
}
