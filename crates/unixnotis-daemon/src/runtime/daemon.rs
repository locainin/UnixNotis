//! Live notification service runtime

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use arc_swap::ArcSwap;
use tokio::sync::watch;
use tracing::{error, info, warn};
use zbus::fdo::DBusProxy;
use zbus::Connection;

use super::shutdown::shutdown_signal;
use crate::child_process::{spawn_center_supervisor, spawn_popups_supervisor};
use crate::cli::Args;
use crate::daemon::{
    log_name_reply, monitor_required_bus_names, request_control_name, request_well_known_name,
    spawn_client_owner_watch, spawn_desktop_index_refresh, verify_name_owner, ControlServer,
    DaemonState, DesktopIdentityIndex, NotificationIngress, NotificationServer,
    NOTIFICATIONS_OBJECT_PATH,
};
use crate::dnd_expiration::DndExpirationScheduler;
use crate::expire::ExpirationScheduler;
use crate::sound::SoundSettings;
use unixnotis_core::{Config, CONTROL_BUS_NAME, CONTROL_OBJECT_PATH, NOTIFICATIONS_BUS_NAME};

pub(super) async fn run_daemon(
    args: &Args,
    config: Config,
    connection: &Connection,
    dbus_proxy: &DBusProxy<'_>,
    desktop_identity_index: Arc<ArcSwap<DesktopIdentityIndex>>,
    watched_desktop_directories: Vec<PathBuf>,
    trusted_test_control_sender: Option<String>,
) -> Result<()> {
    // Resolve sound settings once to avoid repeated filesystem work
    let sound_settings = SoundSettings::from_config(&config, args.config.as_deref());
    let state = DaemonState::new(
        connection.clone(),
        config,
        sound_settings,
        args.trial,
        desktop_identity_index,
    );
    #[cfg(test)]
    state.set_trusted_test_control_sender(trusted_test_control_sender);
    #[cfg(not(test))]
    let _ = trusted_test_control_sender;
    if let Err(error) = spawn_desktop_index_refresh(
        state.desktop_identity_index.clone(),
        watched_desktop_directories,
    ) {
        warn!(?error, "desktop application refresh watcher is unavailable");
    }
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
            NotificationIngress::new(NotificationServer::new(state.clone(), scheduler)),
        )
        .await?;
    connection
        .object_server()
        .at(CONTROL_OBJECT_PATH, ControlServer::new(state.clone()))
        .await?;

    // The standard notification name is the first externally visible readiness gate
    let reply = request_well_known_name(connection, args.trial).await?;
    log_name_reply(&reply);
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
    verify_name_owner(dbus_proxy, connection, NOTIFICATIONS_BUS_NAME).await?;

    // The private control name is published last and means the daemon is ready
    let control_reply = request_control_name(connection).await?;
    match control_reply {
        zbus::fdo::RequestNameReply::PrimaryOwner => {
            info!(CONTROL_BUS_NAME, "acquired control bus name");
        }
        zbus::fdo::RequestNameReply::AlreadyOwner => {
            info!(CONTROL_BUS_NAME, "already owns control bus name");
        }
        zbus::fdo::RequestNameReply::InQueue | zbus::fdo::RequestNameReply::Exists => {
            return Err(anyhow!(
                "control bus name is already owned; another unixnotis instance may be running"
            ));
        }
    }
    verify_name_owner(dbus_proxy, connection, CONTROL_BUS_NAME).await?;

    // A zero-duration run verifies service registration without launching UI processes
    if skip_ui_for_zero_duration(args.run_seconds) {
        info!("zero-duration daemon startup completed");
        return Ok(());
    }

    if let Err(err) = spawn_client_owner_watch(state.clone()).await {
        warn!(?err, "failed to start client owner watcher");
    }

    // Both UI processes share one shutdown flag and reap their current child before exit
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let popups_task = spawn_popups_supervisor(args.clone(), state.clone(), shutdown_rx.clone());
    let center_task = spawn_center_supervisor(args.clone(), state, shutdown_rx);

    info!("unixnotis-daemon running");
    let runtime_result = wait_for_runtime_exit(args.run_seconds, connection.clone()).await;

    if let Err(failure) = &runtime_result {
        error!(
            error = ?failure,
            "session bus connection failed; daemon will exit for supervisor restart"
        );
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
    runtime_result
}

async fn wait_for_runtime_exit(run_seconds: Option<u64>, connection: Connection) -> Result<()> {
    let bus_health = monitor_required_bus_names(connection.clone());
    tokio::pin!(bus_health);
    if let Some(seconds) = run_seconds {
        let timeout = tokio::time::sleep(Duration::from_secs(seconds));
        tokio::select! {
            () = shutdown_signal() => Ok(()),
            result = &mut bus_health => result,
            () = timeout => {
                info!(seconds, "run-seconds elapsed, shutting down");
                Ok(())
            },
        }
    } else {
        tokio::select! {
            () = shutdown_signal() => Ok(()),
            result = &mut bus_health => result,
        }
    }
}

const fn skip_ui_for_zero_duration(run_seconds: Option<u64>) -> bool {
    matches!(run_seconds, Some(0))
}

#[cfg(test)]
#[path = "tests/daemon.rs"]
mod tests;
