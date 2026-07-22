//! Live notification service runtime

use std::time::Duration;

use anyhow::{anyhow, Result};
use tokio::sync::watch;
use tracing::{info, warn};
use zbus::fdo::DBusProxy;
use zbus::Connection;

use super::shutdown::shutdown_signal;
use crate::child_process::{spawn_center_supervisor, spawn_popups_supervisor};
use crate::cli::Args;
use crate::daemon::{
    log_name_reply, request_control_name, request_well_known_name, spawn_inhibitor_owner_watch,
    ControlServer, DaemonState, NotificationServer, NOTIFICATIONS_OBJECT_PATH,
};
use crate::dbus_owner::log_current_owner;
use crate::dnd_expiration::DndExpirationScheduler;
use crate::expire::ExpirationScheduler;
use crate::sound::SoundSettings;
use unixnotis_core::{Config, CONTROL_BUS_NAME, CONTROL_OBJECT_PATH};

pub(super) async fn run_daemon(
    args: &Args,
    config: Config,
    connection: &Connection,
    dbus_proxy: &DBusProxy<'_>,
    notifications_name: zbus::names::BusName<'_>,
) -> Result<()> {
    // Resolve sound settings once to avoid repeated filesystem work
    let sound_settings = SoundSettings::from_config(&config, args.config.as_deref());
    let state = DaemonState::new(connection.clone(), config, sound_settings, args.trial);
    let scheduler = ExpirationScheduler::start(state.clone());
    state.set_scheduler(scheduler.clone());
    let dnd_scheduler = DndExpirationScheduler::start(state.clone());
    state.set_dnd_scheduler(dnd_scheduler);
    let dnd_expires_at = state.store.lock().await.dnd_expires_at();
    state.schedule_dnd_expiration(dnd_expires_at);

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
    let owner_is_self = match log_current_owner(dbus_proxy, connection, notifications_name).await {
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

    // A zero-duration run verifies service registration without launching UI processes
    if skip_ui_for_zero_duration(args.run_seconds) {
        info!("zero-duration daemon startup completed");
        return Ok(());
    }

    if let Err(err) = spawn_inhibitor_owner_watch(state.clone()).await {
        warn!(?err, "failed to start inhibitor owner watcher");
    }

    // Both UI processes share one shutdown flag and reap their current child before exit
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let popups_task = spawn_popups_supervisor(args.clone(), state.clone(), shutdown_rx.clone());
    let center_task = spawn_center_supervisor(args.clone(), state, shutdown_rx);

    info!("unixnotis-daemon running");
    match args.run_seconds {
        Some(seconds) => {
            let timeout = tokio::time::sleep(Duration::from_secs(seconds));
            tokio::select! {
                () = shutdown_signal() => {},
                () = timeout => info!(seconds, "run-seconds elapsed, shutting down"),
            }
        }
        None => shutdown_signal().await,
    }

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

const fn skip_ui_for_zero_duration(run_seconds: Option<u64>) -> bool {
    matches!(run_seconds, Some(0))
}

#[cfg(test)]
#[path = "tests/daemon.rs"]
mod tests;
