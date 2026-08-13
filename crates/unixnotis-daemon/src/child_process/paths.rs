//! Path lookup and spawn setup for supervised UI children

use std::env;
use std::path::PathBuf;

use tokio::process::Command;

#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;

#[cfg(target_os = "linux")]
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
pub(super) fn apply_parent_death_signal(command: &mut Command, expected_parent_pid: u32) {
    // The kernel clears the child relationship before the new program starts
    // SAFETY: This closure only calls prctl through rustix and returns its OS error
    unsafe {
        command.as_std_mut().pre_exec(move || {
            set_parent_process_death_signal(Some(Signal::TERM)).map_err(std::io::Error::from)?;
            let current_parent = rustix::process::getppid()
                .map(|pid| pid.as_raw_nonzero().get())
                .unwrap_or_default();
            if current_parent != i32::try_from(expected_parent_pid).unwrap_or_default() {
                // ESRCH is returned without formatting or allocating after fork
                return Err(std::io::Error::from_raw_os_error(libc::ESRCH));
            }
            Ok(())
        });
    }
}

#[cfg(not(target_os = "linux"))]
pub(super) fn apply_parent_death_signal(_command: &mut Command, _expected_parent_pid: u32) {}

#[cfg(test)]
#[path = "tests/paths.rs"]
mod tests;
