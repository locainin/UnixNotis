//! Application runner for noticenterctl

use anyhow::{Context, Result};
use clap::Parser;
use unixnotis_core::ControlProxy;
use zbus::Connection;

use crate::cli::{Args, Command};

use super::local::handle_local_command;

pub async fn run() -> Result<()> {
    // Parse CLI arguments before any daemon work starts
    let args = Args::parse();
    let command = args.command;

    if let Command::Doctor {
        json,
        verbose,
        service_manager,
        config,
    } = command
    {
        // Doctor owns its D-Bus connection so one failed probe cannot stop later checks
        return crate::doctor::run(json, verbose, service_manager, config).await;
    }

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
