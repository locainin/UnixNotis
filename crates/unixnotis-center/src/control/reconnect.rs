//! Session-bus reconnection and control-generation lifecycle

use std::collections::VecDeque;
use std::future::Future;
use std::time::Duration;

use futures_util::{Stream, StreamExt};
use tokio::sync::mpsc;
use unixnotis_core::{
    ensure_control_api_version, log_session_bus_identity, ControlProxy, CONTROL_BUS_NAME,
    INTERNAL_DBUS_CALL_TIMEOUT,
};
use zbus::fdo::DBusProxy;
use zbus::names::{BusName, UniqueName};
use zbus::proxy::OwnerChangedStream;
use zbus::Connection;

use super::backoff::{
    Backoff, RetryLog, BACKOFF_BASE_MS, BACKOFF_MAX_MS, RETRY_WARN_INTERVAL_SECS,
};
use super::commands::{enqueue_offline_command, stash_offline_commands};
use super::model::{UiCommand, UiEvent};
use super::subscriptions::{run_control_generation, ControlGenerationContext};

#[cfg(test)]
#[path = "tests/reconnect.rs"]
mod reconnect_tests;

pub(super) async fn run_control_loop(
    sender: async_channel::Sender<UiEvent>,
    mut command_rx: mpsc::Receiver<UiCommand>,
) {
    // Buffer UI actions during reconnect to avoid losing user intent
    let mut offline_commands: VecDeque<UiCommand> = VecDeque::new();
    // Connection and subscription failures use separate recovery histories
    let mut connect_backoff = Backoff::new(BACKOFF_BASE_MS, BACKOFF_MAX_MS);
    let mut subscribe_backoff = Backoff::new(BACKOFF_BASE_MS, BACKOFF_MAX_MS);
    let mut connect_log = RetryLog::new(Duration::from_secs(RETRY_WARN_INTERVAL_SECS));
    let mut subscribe_log = RetryLog::new(Duration::from_secs(RETRY_WARN_INTERVAL_SECS));

    loop {
        // No sender means every UI owner is gone and reconnecting would leak this task
        if command_rx.is_closed() {
            return;
        }
        // A disconnected zbus socket is terminal, so each loop owns a fresh bus generation
        let connection = match Connection::session().await {
            Ok(connection) => connection,
            Err(err) => {
                connect_log.warn_or_debug(&err, "failed to connect to session bus; retrying");
                stash_offline_commands(&mut command_rx, &mut offline_commands);
                tokio::time::sleep(connect_backoff.next_sleep()).await;
                continue;
            }
        };
        if let Err(err) = log_session_bus_identity(&connection, "center").await {
            connect_log.warn_or_debug(&err, "session bus identity probe failed; retrying");
            stash_offline_commands(&mut command_rx, &mut offline_commands);
            tokio::time::sleep(connect_backoff.next_sleep()).await;
            continue;
        }
        // A live bus generation clears only connection-level failure history
        connect_backoff.reset();
        connect_log.reset();

        let proxy = match ControlProxy::new(&connection).await {
            Ok(proxy) => proxy,
            Err(err) => {
                connect_log.warn_or_debug(&err, "control interface unavailable, retrying");
                stash_offline_commands(&mut command_rx, &mut offline_commands);
                tokio::time::sleep(connect_backoff.next_sleep()).await;
                continue;
            }
        };
        if let Err(err) = ensure_control_api_version(&proxy).await {
            connect_log.warn_or_debug(&err, "control API version mismatch, retrying");
            stash_offline_commands(&mut command_rx, &mut offline_commands);
            tokio::time::sleep(connect_backoff.next_sleep()).await;
            continue;
        }
        let mut owner_changes = match proxy.inner().receive_owner_changed().await {
            Ok(stream) => stream,
            Err(err) => {
                connect_log.warn_or_debug(&err, "control owner watch unavailable, retrying");
                stash_offline_commands(&mut command_rx, &mut offline_commands);
                tokio::time::sleep(connect_backoff.next_sleep()).await;
                continue;
            }
        };
        let dbus = match DBusProxy::new(&connection).await {
            Ok(proxy) => proxy,
            Err(err) => {
                connect_log.warn_or_debug(&err, "session bus owner proxy unavailable, retrying");
                tokio::time::sleep(connect_backoff.next_sleep()).await;
                continue;
            }
        };

        let owner = match wait_for_control_owner(
            &dbus,
            &mut owner_changes,
            &sender,
            &mut command_rx,
            &mut offline_commands,
        )
        .await
        {
            OwnerWait::Ready(owner) => owner,
            OwnerWait::Disconnected => {
                tokio::time::sleep(connect_backoff.next_sleep()).await;
                continue;
            }
            OwnerWait::Shutdown => return,
        };

        // One generation owns every proxy stream tied to this exact connection
        let generation = run_control_generation(
            &proxy,
            &owner,
            ControlGenerationContext::new(
                &mut owner_changes,
                &sender,
                &mut command_rx,
                &mut offline_commands,
                &mut subscribe_backoff,
                &mut subscribe_log,
            ),
        )
        .await;
        if generation.should_stop() {
            return;
        }
        // Preserve safe commands before replacing the failed generation
        stash_offline_commands(&mut command_rx, &mut offline_commands);
        if generation.requires_connection_backoff() {
            tokio::time::sleep(subscribe_backoff.next_sleep()).await;
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum OwnerWait {
    Ready(String),
    Disconnected,
    Shutdown,
}

async fn wait_for_control_owner(
    dbus: &DBusProxy<'_>,
    owner_changes: &mut OwnerChangedStream<'_>,
    sender: &async_channel::Sender<UiEvent>,
    command_rx: &mut mpsc::Receiver<UiCommand>,
    offline_commands: &mut VecDeque<UiCommand>,
) -> OwnerWait {
    let control_name = BusName::try_from(CONTROL_BUS_NAME)
        .expect("static UnixNotis control bus name must be valid");
    wait_for_control_owner_with_probe(
        || probe_control_owner(dbus, control_name.clone()),
        owner_changes,
        sender,
        command_rx,
        offline_commands,
        BACKOFF_BASE_MS,
    )
    .await
}

#[derive(Debug)]
enum GetOwnerError {
    NoOwner,
    Disconnected(String),
    Transient(String),
}

async fn wait_for_control_owner_with_probe<P, F, S>(
    mut probe: P,
    owner_changes: &mut S,
    sender: &async_channel::Sender<UiEvent>,
    command_rx: &mut mpsc::Receiver<UiCommand>,
    offline_commands: &mut VecDeque<UiCommand>,
    retry_base_ms: u64,
) -> OwnerWait
where
    P: FnMut() -> F,
    F: Future<Output = Result<String, GetOwnerError>>,
    S: Stream<Item = Option<UniqueName<'static>>> + Unpin,
{
    let mut probe_backoff = Backoff::new(retry_base_ms, BACKOFF_MAX_MS);
    let mut probe_log = RetryLog::new(Duration::from_secs(RETRY_WARN_INTERVAL_SECS));
    match probe().await {
        Ok(owner) => return OwnerWait::Ready(owner),
        Err(GetOwnerError::Disconnected(error)) => {
            probe_log.warn_or_debug(&error, "control owner lookup lost its bus connection");
            return OwnerWait::Disconnected;
        }
        Err(GetOwnerError::Transient(error)) => {
            probe_log.warn_or_debug(&error, "control owner lookup failed; retrying");
        }
        Err(GetOwnerError::NoOwner) => {}
    }
    // Missing ownership is a stable disconnected state, not a connection failure
    let _ = sender.send(UiEvent::Disconnected).await;
    loop {
        let retry_delay = probe_backoff.next_sleep();
        tokio::select! {
            command = command_rx.recv() => {
                let Some(command) = command else {
                    return OwnerWait::Shutdown;
                };
                enqueue_offline_command(offline_commands, command);
            }
            update = owner_changes.next() => {
                match update {
                    Some(Some(owner)) => return OwnerWait::Ready(owner.to_string()),
                    Some(None) => {}
                    None => return OwnerWait::Disconnected,
                }
            }
            () = tokio::time::sleep(retry_delay) => {
                match probe().await {
                    Ok(owner) => return OwnerWait::Ready(owner),
                    Err(GetOwnerError::NoOwner) => {}
                    Err(GetOwnerError::Disconnected(error)) => {
                        probe_log.warn_or_debug(
                            &error,
                            "control owner lookup lost its bus connection",
                        );
                        return OwnerWait::Disconnected;
                    }
                    Err(GetOwnerError::Transient(error)) => {
                        probe_log.warn_or_debug(
                            &error,
                            "control owner lookup failed; retrying",
                        );
                    }
                }
            }
        }
    }
}

async fn probe_control_owner(
    dbus: &DBusProxy<'_>,
    control_name: BusName<'_>,
) -> Result<String, GetOwnerError> {
    match tokio::time::timeout(
        INTERNAL_DBUS_CALL_TIMEOUT,
        dbus.get_name_owner(control_name),
    )
    .await
    {
        Ok(Ok(owner)) => Ok(owner.to_string()),
        Ok(Err(zbus::fdo::Error::NameHasNoOwner(_))) => Err(GetOwnerError::NoOwner),
        Ok(Err(error)) if owner_error_is_disconnected(&error) => {
            Err(GetOwnerError::Disconnected(error.to_string()))
        }
        Ok(Err(error)) => Err(GetOwnerError::Transient(error.to_string())),
        Err(_) => Err(GetOwnerError::Transient(
            "control owner lookup timed out".to_string(),
        )),
    }
}

const fn owner_error_is_disconnected(error: &zbus::fdo::Error) -> bool {
    matches!(
        error,
        zbus::fdo::Error::IOError(_)
            | zbus::fdo::Error::NoServer(_)
            | zbus::fdo::Error::NoNetwork(_)
            | zbus::fdo::Error::ZBus(zbus::Error::InputOutput(_))
    )
}
