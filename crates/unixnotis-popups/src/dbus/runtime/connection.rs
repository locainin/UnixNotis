//! Session-bus recovery and control-owner discovery

use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::{mpsc, watch};
use tracing::warn;
use unixnotis_core::{
    ensure_control_api_version, log_session_bus_identity, ControlProxy, CONTROL_BUS_NAME,
    INTERNAL_DBUS_CALL_TIMEOUT,
};
use zbus::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::proxy::OwnerChangedStream;
use zbus::Connection;

use super::generation::{run_owner_generation, GenerationExit, PopupGenerationContext};
use crate::dbus::backoff::{
    Backoff, RetryLog, BACKOFF_BASE_MS, BACKOFF_MAX_MS, RETRY_WARN_INTERVAL_SECS,
};
use crate::dbus::commands::drain_offline_commands;
use crate::dbus::{UiCommand, UiEvent};

pub(super) async fn run_dbus_loop(
    sender: async_channel::Sender<UiEvent>,
    mut command_rx: mpsc::Receiver<UiCommand>,
    mut gtk_ready_rx: watch::Receiver<bool>,
) {
    let mut connect_backoff = Backoff::new(BACKOFF_BASE_MS, BACKOFF_MAX_MS);
    let mut subscribe_backoff = Backoff::new(BACKOFF_BASE_MS, BACKOFF_MAX_MS);
    let mut connect_log = RetryLog::new(Duration::from_secs(RETRY_WARN_INTERVAL_SECS));
    let mut subscribe_log = RetryLog::new(Duration::from_secs(RETRY_WARN_INTERVAL_SECS));

    loop {
        let connection = connect_session_bus(&mut connect_backoff, &mut connect_log).await;
        let Some(retry_delay) = run_connection_once(
            &connection,
            &sender,
            &mut command_rx,
            &mut subscribe_backoff,
            &mut subscribe_log,
            &mut gtk_ready_rx,
        )
        .await
        else {
            return;
        };
        tokio::time::sleep(retry_delay).await;
    }
}

async fn connect_session_bus(
    connect_backoff: &mut Backoff,
    connect_log: &mut RetryLog,
) -> Connection {
    loop {
        match Connection::session().await {
            Ok(connection) => {
                if let Err(error) = log_session_bus_identity(&connection, "popups").await {
                    connect_log
                        .warn_or_debug(&error, "session bus identity probe failed; retrying");
                    tokio::time::sleep(connect_backoff.next_sleep()).await;
                    continue;
                }
                connect_backoff.reset();
                connect_log.reset();
                return connection;
            }
            Err(error) => {
                connect_log.warn_or_debug(&error, "failed to connect to the session bus; retrying");
                tokio::time::sleep(connect_backoff.next_sleep()).await;
            }
        }
    }
}

async fn run_connection_once(
    connection: &Connection,
    sender: &async_channel::Sender<UiEvent>,
    command_rx: &mut mpsc::Receiver<UiCommand>,
    subscribe_backoff: &mut Backoff,
    subscribe_log: &mut RetryLog,
    gtk_ready_rx: &mut watch::Receiver<bool>,
) -> Option<Duration> {
    let proxy = match ControlProxy::new(connection).await {
        Ok(proxy) => proxy,
        Err(error) => {
            subscribe_log.warn_or_debug(&error, "control interface unavailable; retrying");
            if acknowledge_offline_shutdown(command_rx) {
                return None;
            }
            return Some(subscribe_backoff.next_sleep());
        }
    };
    if let Err(error) = ensure_control_api_version(&proxy).await {
        subscribe_log.warn_or_debug(&error, "control API version mismatch; retrying");
        return Some(subscribe_backoff.next_sleep());
    }
    let mut owner_changes = match proxy.inner().receive_owner_changed().await {
        Ok(stream) => stream,
        Err(error) => {
            subscribe_log.warn_or_debug(&error, "control owner watch unavailable; retrying");
            return Some(subscribe_backoff.next_sleep());
        }
    };
    let dbus = match DBusProxy::new(connection).await {
        Ok(proxy) => proxy,
        Err(error) => {
            subscribe_log.warn_or_debug(&error, "session owner proxy unavailable; retrying");
            return Some(subscribe_backoff.next_sleep());
        }
    };

    loop {
        let owner =
            match wait_for_control_owner(&dbus, &mut owner_changes, sender, command_rx).await {
                OwnerWait::Ready(owner) => owner,
                OwnerWait::Disconnected => return Some(subscribe_backoff.next_sleep()),
                OwnerWait::Shutdown => return None,
            };
        let context = PopupGenerationContext::new(
            &mut owner_changes,
            sender,
            command_rx,
            subscribe_backoff,
            subscribe_log,
            gtk_ready_rx,
        );
        match run_owner_generation(&proxy, &owner, context).await {
            GenerationExit::OwnerChanged => {}
            GenerationExit::ConnectionLost => return Some(subscribe_backoff.next_sleep()),
            GenerationExit::Shutdown => return None,
            GenerationExit::Retry => {
                tokio::time::sleep(subscribe_backoff.next_sleep()).await;
            }
        }
    }
}

pub(super) enum OwnerWait {
    Ready(String),
    Disconnected,
    Shutdown,
}

async fn wait_for_control_owner(
    dbus: &DBusProxy<'_>,
    owner_changes: &mut OwnerChangedStream<'_>,
    sender: &async_channel::Sender<UiEvent>,
    command_rx: &mut mpsc::Receiver<UiCommand>,
) -> OwnerWait {
    let control_name =
        BusName::try_from(CONTROL_BUS_NAME).expect("static control bus name must be valid");
    if let Ok(Ok(owner)) = tokio::time::timeout(
        INTERNAL_DBUS_CALL_TIMEOUT,
        dbus.get_name_owner(control_name),
    )
    .await
    {
        return OwnerWait::Ready(owner.to_string());
    }

    // An unowned name is a quiet state and must not trigger seed or readiness calls
    let _ = sender.send(UiEvent::Disconnected).await;
    if acknowledge_offline_shutdown(command_rx) {
        return OwnerWait::Shutdown;
    }
    loop {
        tokio::select! {
            command = command_rx.recv() => {
                match command {
                    Some(UiCommand::Shutdown(acknowledgement)) => {
                        let _ = acknowledgement.send(());
                        return OwnerWait::Shutdown;
                    }
                    Some(_) => warn!("dropping popup command while control has no owner"),
                    None => return OwnerWait::Shutdown,
                }
            }
            update = owner_changes.next() => {
                match update {
                    Some(Some(owner)) => return OwnerWait::Ready(owner.to_string()),
                    Some(None) => {}
                    None => return OwnerWait::Disconnected,
                }
            }
        }
    }
}

fn acknowledge_offline_shutdown(command_rx: &mut mpsc::Receiver<UiCommand>) -> bool {
    if let Some(acknowledgement) = drain_offline_commands(command_rx) {
        let _ = acknowledgement.send(());
        true
    } else {
        false
    }
}
