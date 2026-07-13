//! Daemon entrypoint and service bootstrap

#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::manual_let_else,
    clippy::needless_continue,
    clippy::needless_pass_by_ref_mut,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::ref_option,
    clippy::significant_drop_tightening,
    clippy::struct_excessive_bools,
    clippy::struct_field_names,
    clippy::trivially_copy_pass_by_ref,
    clippy::unnecessary_wraps,
    clippy::unused_async,
    reason = "reviewed D-Bus trait signatures, lock lifetimes, protocol integer widths, and private-module visibility preserve daemon compatibility"
)]

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, ValueEnum};
use tokio::sync::watch;
use tracing::{info, warn};
use zbus::fdo::DBusProxy;
use zbus::Connection;

#[path = "child_process/root.rs"]
mod child_process;
#[path = "daemon/root.rs"]
mod daemon;
#[path = "dbus_owner.rs"]
mod dbus_owner;
mod expire;
#[cfg(test)]
#[path = "tests/module_wiring.rs"]
mod module_wiring_tests;
#[path = "runtime_config.rs"]
mod runtime_config;
#[path = "shutdown_signal.rs"]
mod shutdown_signal;
#[path = "sound/root.rs"]
mod sound;
mod store;
mod system_tools;
#[cfg(test)]
#[path = "tests/support.rs"]
mod test_support;
#[path = "trial_mode/root.rs"]
mod trial_mode;

use crate::child_process::{spawn_center_supervisor, spawn_popups_supervisor};
use crate::daemon::{
    log_name_reply, request_control_name, request_well_known_name, spawn_inhibitor_owner_watch,
    ControlServer, DaemonState, NotificationServer,
};
use crate::dbus_owner::{log_current_owner, wait_for_owner_state};
use crate::expire::ExpirationScheduler;
use crate::runtime_config::{ensure_wayland_session, init_tracing, load_config};
use crate::shutdown_signal::shutdown_signal;
use crate::sound::SoundSettings;
use crate::trial_mode::{prepare_trial, restore_previous, TrialState};
use unixnotis_core::{Config, CONTROL_BUS_NAME, CONTROL_OBJECT_PATH};

const NOTIFICATIONS_OBJECT_PATH: &str = "/org/freedesktop/Notifications";

#[derive(Parser, Debug, Clone)]
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

    /// Exit after running for the requested number of seconds (profiling helper)
    #[arg(long)]
    run_seconds: Option<u64>,
}

#[derive(Clone, Debug, ValueEnum)]
enum RestoreStrategy {
    Auto,
    None,
    Systemd,
    Process,
}

#[cfg(test)]
#[path = "tests/main_args.rs"]
mod main_args_tests;

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

    ensure_wayland_session(Duration::from_secs(20))
        .await
        .context("wait for Wayland session")?;

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

    // Trial cleanup runs after every daemon result, including partial startup failures
    let run_result = run_daemon(
        &args,
        config,
        &connection,
        &dbus_proxy,
        notifications_name.clone(),
    )
    .await;
    let restore_result = finish_trial(
        &args,
        &connection,
        &dbus_proxy,
        notifications_name,
        &mut trial_state,
    )
    .await;
    combine_run_and_restore(run_result, restore_result)
}

async fn run_daemon(
    args: &Args,
    config: Config,
    connection: &Connection,
    dbus_proxy: &DBusProxy<'_>,
    notifications_name: zbus::names::BusName<'_>,
) -> Result<()> {
    // Resolve sound settings once to avoid repeated filesystem work
    let sound_settings = SoundSettings::from_config(&config);
    let state = DaemonState::new(connection.clone(), config, sound_settings, args.trial);
    let scheduler = ExpirationScheduler::start(state.clone());
    // Close and clear paths need the scheduler handle so timers can be canceled early
    state.set_scheduler(scheduler.clone());

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

    let control_reply = request_control_name(connection).await?;
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

    let reply = request_well_known_name(connection, args.trial).await?;
    log_name_reply(&reply);
    let owner_is_self =
        match log_current_owner(dbus_proxy, connection, notifications_name.clone()).await {
            Ok(value) => value,
            Err(err) => {
                warn!(?err, "failed to query current notification owner");
                false
            }
        };
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

    if let Err(err) = spawn_inhibitor_owner_watch(state.clone()).await {
        warn!(?err, "failed to start inhibitor owner watcher");
    }

    // Each UI child runs under a small supervisor loop
    // This keeps Wayland disconnects from leaving dead zombies behind
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let popups_task = spawn_popups_supervisor(args.clone(), state.clone(), shutdown_rx.clone());
    let center_task = spawn_center_supervisor(args.clone(), state.clone(), shutdown_rx);

    info!("unixnotis-daemon running");
    match args.run_seconds {
        Some(seconds) => {
            let timeout = tokio::time::sleep(Duration::from_secs(seconds));
            tokio::select! {
                () = shutdown_signal() => {},
                () = timeout => {
                    info!(seconds, "run-seconds elapsed, shutting down");
                }
            }
        }
        None => {
            shutdown_signal().await;
        }
    }

    // A shared flag lets both supervisors stop and reap their current child cleanly
    if let Err(err) = shutdown_tx.send(true) {
        warn!(?err, "shutdown signal receivers already closed");
    }
    if let Err(err) = popups_task.await {
        warn!(?err, "popups supervisor task failed");
    }
    if let Err(err) = center_task.await {
        warn!(?err, "center supervisor task failed");
    }

    Ok(())
}

async fn finish_trial(
    args: &Args,
    connection: &Connection,
    dbus_proxy: &DBusProxy<'_>,
    notifications_name: zbus::names::BusName<'_>,
    trial_state: &mut TrialState,
) -> Result<()> {
    if !args.trial {
        return Ok(());
    }

    // Releasing the name and restarting the prior owner are independent cleanup duties
    let release_result = connection
        .release_name("org.freedesktop.Notifications")
        .await
        .context("release org.freedesktop.Notifications after trial")
        .map(|_| ());
    let restore_result = restore_trial_owner(
        args,
        dbus_proxy,
        notifications_name,
        trial_state.take_restore_action(),
    )
    .await;

    combine_run_and_restore(release_result, restore_result)
}

async fn restore_trial_owner(
    args: &Args,
    dbus_proxy: &DBusProxy<'_>,
    notifications_name: zbus::names::BusName<'_>,
    action: Option<trial_mode::RestoreAction>,
) -> Result<()> {
    let Some(action) = action else {
        return Ok(());
    };

    restore_previous_or_fail(action)?;
    let reacquired = wait_for_owner_state(
        dbus_proxy,
        notifications_name,
        true,
        Duration::from_millis(args.restore_wait_ms),
    )
    .await
    .context("wait for previous daemon to reacquire org.freedesktop.Notifications")?;
    if !reacquired {
        anyhow::bail!(
            "previous daemon did not reacquire org.freedesktop.Notifications within {} ms",
            args.restore_wait_ms
        );
    }
    Ok(())
}

fn combine_run_and_restore(run_result: Result<()>, restore_result: Result<()>) -> Result<()> {
    match (run_result, restore_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(run_error), Ok(())) => Err(run_error),
        (Ok(()), Err(restore_error)) => Err(restore_error),
        (Err(run_error), Err(restore_error)) => {
            Err(run_error.context(format!("trial restoration also failed: {restore_error:#}")))
        }
    }
}

fn restore_previous_or_fail(action: trial_mode::RestoreAction) -> Result<()> {
    restore_previous(action).context("restore previous notification daemon")
}
