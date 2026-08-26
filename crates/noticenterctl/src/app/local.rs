//! Local command dispatch that does not require a running daemon

use anyhow::{Context, Result};

use crate::cli::{Command, DevCommand, DoctorCommand};

pub(super) fn handle_local_command(command: Command) -> Result<()> {
    // Local commands remain available while the session bus or daemon is unavailable
    match command {
        Command::CssCheck { config } => crate::css_check::run(config),
        Command::Preset { command } => {
            crate::preset::run_preset(command).context("preset command failed")
        }
        Command::Theme { command } => crate::theme::run(command).context("theme command failed"),
        Command::Doctor {
            command: Some(DoctorCommand::RepairSession),
            service_manager,
            ..
        } => crate::session_environment::sync(service_manager),
        Command::Dev {
            command: DevCommand::Logs,
        } => crate::debug_logs::follow_debug_logs(),
        // Incorrect routing must fail instead of reporting a successful no-op
        other => anyhow::bail!("internal routing error: {other:?} is not a local command"),
    }
}

#[cfg(test)]
#[path = "tests/local.rs"]
mod tests;
