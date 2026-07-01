//! Cargo build and debug-binary resolution for trial mode

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

// Trial mode needs every runtime binary that may be spawned by the daemon
const TRIAL_PACKAGES: [&str; 4] = [
    "unixnotis-daemon",
    "unixnotis-popups",
    "unixnotis-center",
    "noticenterctl",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TrialBinaries {
    // The daemon is launched directly so no user service manager is touched
    pub(super) daemon: PathBuf,
    // The control binary is exposed either directly or through a temporary shim
    pub(super) control: PathBuf,
}

pub(super) fn build_trial_binaries(repo_root: &Path) -> Result<TrialBinaries> {
    // Build every runtime binary before launch so stale debug outputs are not reused
    let mut command = std::process::Command::new("cargo");
    command.arg("build");
    for package in TRIAL_PACKAGES {
        // Package arguments stay explicit so adding a runtime binary is visible here
        command.arg("-p").arg(package);
    }
    let build_status = command
        .current_dir(repo_root)
        .status()
        .map_err(|err| anyhow!("failed to build trial binaries: {}", err))?;
    if !build_status.success() {
        // A failed build should never fall through into an older target/debug binary
        return Err(anyhow!("trial build exited with failure"));
    }

    let binaries = trial_binary_paths(repo_root);
    ensure_trial_binary(&binaries.daemon, "trial daemon")?;
    ensure_trial_binary(&binaries.control, "trial control")?;
    Ok(binaries)
}

pub(super) fn trial_binary_paths(repo_root: &Path) -> TrialBinaries {
    // Trial mode intentionally uses debug outputs to keep local edit-test cycles fast
    let debug_dir = repo_root.join("target").join("debug");
    TrialBinaries {
        daemon: debug_dir.join("unixnotis-daemon"),
        control: debug_dir.join("noticenterctl"),
    }
}

fn ensure_trial_binary(path: &Path, label: &str) -> Result<()> {
    if path.is_file() {
        return Ok(());
    }

    // Missing output usually means Cargo produced a different target tree or failed early
    Err(anyhow!("{} binary not found at {}", label, path.display()))
}
