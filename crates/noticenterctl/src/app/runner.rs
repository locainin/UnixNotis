//! Application runner for noticenterctl

use anyhow::{Context, Result};
use clap::Parser;
use unixnotis_core::{ensure_control_api_version, log_session_bus_identity, ControlProxy};
use zbus::Connection;

use crate::cli::{Args, Command, ExecutionKind};

use super::local::handle_local_command;

pub fn run() -> Result<()> {
    // Parse CLI arguments before any daemon work starts
    let args = Args::parse();
    run_command(args.command)
}

pub(super) fn run_command(command: Command) -> Result<()> {
    // Semantic checks happen before runtime and D-Bus setup
    command.validate()?;

    match command.execution_kind() {
        ExecutionKind::LocalSync => handle_local_command(command),
        ExecutionKind::LocalAsync | ExecutionKind::Daemon => {
            // A current-thread runtime avoids a worker pool for short control commands
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .context("build command runtime")?;
            runtime.block_on(run_async(command))
        }
    }
}

pub(super) async fn run_async(command: Command) -> Result<()> {
    match command {
        Command::Doctor {
            command: None,
            json,
            verbose,
            service_manager,
            config,
        } => {
            // Doctor owns its D-Bus connection so one failed probe cannot stop later checks
            crate::doctor::run(json, verbose, service_manager, config).await
        }
        Command::Doctor {
            command: Some(_), ..
        } => anyhow::bail!("internal routing error: doctor repair reached async dispatcher"),
        command => run_daemon(command).await,
    }
}

async fn run_daemon(command: Command) -> Result<()> {
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
    ensure_control_api_version(&proxy)
        .await
        .context("validate UnixNotis component version")?;

    crate::dbus::handle_command(&proxy, command).await
}
