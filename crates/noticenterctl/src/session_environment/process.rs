//! Typed service-manager command execution through trusted tool lookup

use std::process::ExitStatus;

use anyhow::{bail, Context, Result};
use unixnotis_core::CommandSpec;

use crate::system_tools;

pub(super) fn run(command: &CommandSpec) -> Result<ExitStatus> {
    // Trusted lookup prevents inherited PATH entries from selecting service tools
    let mut process = system_tools::command_from_spec(command)
        .with_context(|| format!("resolve trusted {} executable", command.display_lossy()))?;
    process.status().with_context(|| {
        format!(
            "run {} for session environment sync",
            command.display_lossy()
        )
    })
}

pub(super) fn require_success(command: &CommandSpec) -> Result<()> {
    // Preserve the native exit status in the user-facing failure report
    let status = run(command)?;
    if !status.success() {
        bail!("{} exited with status {status}", command.display_lossy());
    }
    Ok(())
}
