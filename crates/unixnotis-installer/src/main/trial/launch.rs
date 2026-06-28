//! Trial process launch and signal-time cleanup shell rendering

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use super::build::build_trial_binaries;
use super::paths::shell_quote;
use super::shim::ensure_trial_control_access;

const TRIAL_DAEMON_ARGS: [&str; 4] = ["--trial", "--restore", "auto", "--yes"];

pub(crate) fn run_trial(repo_root: PathBuf) -> Result<()> {
    println!("Starting UnixNotis trial run.");
    println!("Press Ctrl+C to stop and restore the previous daemon.");

    let binaries = build_trial_binaries(&repo_root)?;
    println!("Trial control binary: {}", binaries.control.display());

    // A temporary PATH shim is optional; direct control-binary usage remains valid
    let trial_ctl_shim = ensure_trial_control_access(&binaries.control)?;
    let status = if let Some(shim) = trial_ctl_shim.as_ref() {
        // Shell trap cleanup covers signal paths where Rust Drop may not run
        run_trial_with_shim_cleanup(&binaries.daemon, &shim.path, &shim.target)?
    } else {
        // No shim means the daemon can be launched directly without filesystem cleanup
        run_trial_process(&binaries.daemon)?
    };

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("trial run exited with failure"))
    }
}

fn run_trial_process(daemon_bin: &Path) -> Result<std::process::ExitStatus> {
    // The daemon performs the actual restore behavior after the trial exits
    std::process::Command::new(daemon_bin)
        .args(TRIAL_DAEMON_ARGS)
        .status()
        .map_err(|err| anyhow!("failed to run trial: {}", err))
}

fn run_trial_with_shim_cleanup(
    daemon_bin: &Path,
    shim_path: &Path,
    expected_target: &Path,
) -> Result<std::process::ExitStatus> {
    // Quote paths before rendering shell so spaces and quotes stay literal
    let daemon = shell_quote(daemon_bin.display().to_string().as_str());
    let shim = shell_quote(shim_path.display().to_string().as_str());
    let target = shell_quote(expected_target.display().to_string().as_str());
    let script = trial_launch_script(&daemon, &shim, &target);
    std::process::Command::new("sh")
        .arg("-c")
        .arg(script)
        .status()
        .map_err(|err| anyhow!("failed to run trial: {}", err))
}

pub(super) fn trial_launch_script(daemon: &str, shim: &str, target: &str) -> String {
    // Cleanup is guarded by both symlink type and expected target
    format!(
        "cleanup() {{ if [ -L {shim} ] && [ \"$(readlink -- {shim})\" = {target} ]; then rm -f -- {shim}; fi; }}; trap cleanup EXIT INT TERM; {daemon} --trial --restore auto --yes"
    )
}
