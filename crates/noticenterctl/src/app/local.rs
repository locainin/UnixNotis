//! Local command dispatch that does not require a running daemon

use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::cli::{Command, PresetCommand};

pub(super) fn handle_local_command(
    command: Command,
    mut run_css: impl FnMut(Option<PathBuf>) -> Result<()>,
    mut run_preset: impl FnMut(PresetCommand) -> Result<()>,
) -> Result<()> {
    // Local commands remain available while the session bus or daemon is unavailable
    match command {
        Command::CssCheck { config } => run_css(config),
        Command::Preset { command } => run_preset(command).context("preset command failed"),
        // The caller routes daemon-backed commands before reaching this helper
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "tests/local.rs"]
mod tests;
