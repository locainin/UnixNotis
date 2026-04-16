//! Path lookup and spawn setup for supervised UI children

use std::env;
use std::path::PathBuf;

use tokio::process::Command;

#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;

#[cfg(unix)]
use rustix::process::{set_parent_process_death_signal, Signal};

fn resolve_sibling_binary(name: &str) -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let dir = exe.parent()?;

    // Prefer sibling binaries next to the daemon binary
    // This keeps local installs working without a PATH lookup
    let candidate = dir.join(name);
    if candidate.is_file() {
        return Some(candidate);
    }

    // Windows-style suffix support keeps mixed developer setups simple
    let candidate = dir.join(format!("{name}.exe"));
    if candidate.is_file() {
        return Some(candidate);
    }
    None
}

pub(super) fn resolve_popups_path() -> Option<PathBuf> {
    resolve_sibling_binary("unixnotis-popups")
}

pub(super) fn resolve_center_path() -> Option<PathBuf> {
    resolve_sibling_binary("unixnotis-center")
}

#[cfg(target_os = "linux")]
pub(super) fn apply_parent_death_signal(command: &mut Command) {
    // If the daemon dies, the UI child should not linger alone
    unsafe {
        command.as_std_mut().pre_exec(|| {
            set_parent_process_death_signal(Some(Signal::TERM)).map_err(std::io::Error::from)
        });
    }
}

#[cfg(not(target_os = "linux"))]
pub(super) fn apply_parent_death_signal(_command: &mut Command) {}
