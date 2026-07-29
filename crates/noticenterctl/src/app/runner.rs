//! Application runner for noticenterctl

use anyhow::{Context, Result};
use clap::Parser;
use unixnotis_core::{log_session_bus_identity, ControlProxy};
use zbus::Connection;

use crate::cli::{Args, Command};

use super::local::handle_local_command;

pub fn run() -> Result<()> {
    // Parse CLI arguments before any daemon work starts
    let args = Args::parse();
    let command = args.command;
    // Semantic checks happen before runtime and D-Bus setup
    command.validate()?;

    if command.is_synchronous() {
        // Preset and CSS work should not pay for an unused asynchronous runtime
        handle_local_command(
            command,
            crate::css_check::run,
            crate::preset::run_preset,
            crate::session_environment::sync,
            crate::theme::run,
        )?;
        return Ok(());
    }

    // A current-thread runtime avoids a worker pool for short control commands
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build command runtime")?;
    runtime.block_on(run_async(command))
}

async fn run_async(command: Command) -> Result<()> {
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

    // Every remaining command is backed by the running daemon
    debug_assert!(
        !command.is_local_only(),
        "local-only commands must return before D-Bus dispatch"
    );

    // Control commands need the session bus and the daemon proxy
    let connection = Connection::session()
        .await
        .context("connect to session bus")?;
    log_session_bus_identity(&connection, "noticenterctl")
        .await
        .context("read noticenterctl session-bus identity")?;
    let proxy = ControlProxy::new(&connection)
        .await
        .context("connect to unixnotis control interface")?;

    crate::dbus::handle_command(&proxy, command).await
}
