//! Application runner for noticenterctl

use anyhow::{Context, Result};
use clap::Parser;
use unixnotis_core::ControlProxy;
use zbus::Connection;

use crate::cli::{Args, Command};

pub async fn run() -> Result<()> {
    // Parse CLI arguments before any daemon work starts
    let args = Args::parse();
    let command = args.command;

    if command.is_local_only() {
        // Local commands must work even when the daemon is not running
        handle_local_command(command, crate::css_check::run, crate::preset::run_preset)?;
        return Ok(());
    }

    // Control commands need the session bus and the daemon proxy
    let connection = Connection::session()
        .await
        .context("connect to session bus")?;
    let proxy = ControlProxy::new(&connection)
        .await
        .context("connect to unixnotis control interface")?;

    crate::dbus::handle_command(&proxy, command).await
}

fn handle_local_command(
    command: Command,
    mut run_css: impl FnMut() -> Result<()>,
    mut run_preset: impl FnMut(crate::cli::PresetCommand) -> Result<()>,
) -> Result<()> {
    match command {
        Command::CssCheck => run_css(),
        Command::Preset { command } => run_preset(command).context("preset command failed"),
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "app/tests/command.rs"]
mod tests;
