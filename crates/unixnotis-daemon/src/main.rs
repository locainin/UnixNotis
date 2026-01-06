//! Daemon entrypoint and service bootstrap.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, ValueEnum};
use tracing::{error, info};
use zbus::fdo::DBusProxy;
use zbus::Connection;

mod daemon;
mod expire;
mod sound;
mod store;
#[path = "runtime_config.rs"]
mod runtime_config;
#[path = "dbus_owner.rs"]
mod dbus_owner;
#[path = "child_process.rs"]
mod child_process;
#[path = "shutdown_signal.rs"]
mod shutdown_signal;
#[path = "trial_mode.rs"]
mod trial_mode;

use crate::daemon::{
    log_name_reply, request_control_name, request_well_known_name, ControlServer, DaemonState,
    NotificationServer,
};
use crate::expire::ExpirationScheduler;
use crate::runtime_config::{init_tracing, is_wayland_session, load_config};
use crate::dbus_owner::{log_current_owner, wait_for_owner_state};
use crate::child_process::{
    start_center_process, start_popups_process, stop_center_process, stop_popups_process,
};
use crate::shutdown_signal::shutdown_signal;
use crate::trial_mode::{prepare_trial, restore_previous, TrialState};
use crate::sound::SoundSettings;
use unixnotis_core::{Config, CONTROL_BUS_NAME, CONTROL_OBJECT_PATH};

const NOTIFICATIONS_OBJECT_PATH: &str = "/org/freedesktop/Notifications";

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Path to config.toml
    #[arg(long)]
    config: Option<PathBuf>,

    /// Run in trial mode and replace any existing daemon
    #[arg(long)]
    trial: bool,

    /// Restore strategy after trial mode ends
    #[arg(long, value_enum, default_value_t = RestoreStrategy::Auto)]
    restore: RestoreStrategy,

    /// Skip confirmation prompt in trial mode
    #[arg(long)]
    yes: bool,

    /// Time to wait for another daemon to re-acquire after release (ms)
    #[arg(long, default_value_t = 2000)]
    restore_wait_ms: u64,

    /// Validate configuration and exit
    #[arg(long)]
    check: bool,
}

#[derive(Clone, Debug, ValueEnum)]
enum RestoreStrategy {
    Auto,
    None,
    Systemd,
    Process,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config = load_config(&args).context("load config")?;

    init_tracing(&config);
    let config_source = if args.config.is_some() {
        "custom"
    } else {
        match Config::default_config_path() {
            Ok(path) if path.exists() => "default",
            _ => "builtin",
        }
    };
    info!(config_source, "configuration loaded");
    if unixnotis_core::util::diagnostic_mode() {
        info!(
            limit = unixnotis_core::util::log_limit(),
            "diagnostic logging enabled (snippets capped; newlines stripped)"
        );
    }

    if args.check {
        info!("configuration loaded successfully");
        return Ok(());
    }

    if !is_wayland_session() {
        return Err(anyhow!(
            "Wayland session not detected; use --check for config validation"
        ));
    }

    let connection = Connection::session()
        .await
        .context("connect to session bus")?;
    let dbus_proxy = DBusProxy::new(&connection).await?;
    let notifications_name = zbus::names::BusName::try_from("org.freedesktop.Notifications")?;

    let mut trial_state = if args.trial {
        prepare_trial(&args, &dbus_proxy, notifications_name.clone()).await?
    } else {
        TrialState::default()
    };

    // Resolve sound settings once to avoid repeated filesystem work.
    let sound_settings = SoundSettings::from_config(&config);
    let state = DaemonState::new(connection.clone(), config, sound_settings);
    let scheduler = ExpirationScheduler::start(state.clone());

    connection
        .object_server()
        .at(
            NOTIFICATIONS_OBJECT_PATH,
            NotificationServer::new(state.clone(), scheduler),
        )
        .await?;
    connection
        .object_server()
        .at(CONTROL_OBJECT_PATH, ControlServer::new(state.clone()))
        .await?;

    let control_reply = request_control_name(&connection).await?;
    match control_reply {
        zbus::fdo::RequestNameReply::PrimaryOwner => {
            info!(CONTROL_BUS_NAME, "acquired control bus name");
        }
        zbus::fdo::RequestNameReply::AlreadyOwner => {
            info!(CONTROL_BUS_NAME, "already owns control bus name");
        }
        _ => {
            return Err(anyhow!(
                "control bus name is already owned; another unixnotis instance may be running"
            ));
        }
    }

    let reply = request_well_known_name(&connection, args.trial).await?;
    log_name_reply(&reply);
    let owner_is_self = log_current_owner(&dbus_proxy, &connection, notifications_name.clone())
        .await
        .unwrap_or(false);
    if !args.trial
        && !matches!(
            reply,
            zbus::fdo::RequestNameReply::PrimaryOwner | zbus::fdo::RequestNameReply::AlreadyOwner
        )
    {
        return Err(anyhow!(
            "org.freedesktop.Notifications is already owned; retry with --trial"
        ));
    }
    if args.trial && !owner_is_self {
        return Err(anyhow!(
            "org.freedesktop.Notifications is still owned by another daemon; stop it or use --restore systemd if managed by systemd --user"
        ));
    }

    let mut popups_process = start_popups_process(&args)?;
    let mut center_process = start_center_process(&args)?;

    info!("unixnotis-daemon running");
    shutdown_signal().await;

    if let Some(mut child) = popups_process.take() {
        stop_popups_process(&mut child);
    }
    if let Some(mut child) = center_process.take() {
        stop_center_process(&mut child);
    }

    if args.trial {
        if let Err(err) = connection
            .release_name("org.freedesktop.Notifications")
            .await
        {
            error!(?err, "failed to release notification name");
        }

        if let Some(action) = trial_state.take_restore_action() {
            if let Err(err) = restore_previous(action) {
                error!(?err, "failed to restore previous notification daemon");
            }
            let reacquired = wait_for_owner_state(
                &dbus_proxy,
                zbus::names::BusName::try_from("org.freedesktop.Notifications")?,
                true,
                Duration::from_millis(args.restore_wait_ms),
            )
            .await
            .unwrap_or(false);

            if !reacquired {
                info!("no daemon re-acquired after release");
            }
        }
    }

    Ok(())
}
