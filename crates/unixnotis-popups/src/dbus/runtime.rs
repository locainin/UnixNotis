//! Popup D-Bus runtime bootstrap and stream loop

use std::thread;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::mpsc;
use tracing::{info, warn};
use unixnotis_core::{
    log_session_bus_identity, timed_dbus_call, ControlProxy, CONTROL_BUS_NAME,
    INTERNAL_DBUS_CALL_TIMEOUT,
};
use zbus::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::proxy::OwnerChangedStream;
use zbus::Connection;

use super::backoff::{
    Backoff, RetryLog, BACKOFF_BASE_MS, BACKOFF_MAX_MS, RETRY_WARN_INTERVAL_SECS,
};
use super::commands::{drain_offline_commands, handle_command};
use super::seed::{seed_state, PopupSeedSource, SeedError, SeedSnapshot};
use super::types::{UiCommand, UiEvent};

// Bound UI commands to avoid unbounded memory growth under a stuck UI event loop
const UI_COMMAND_QUEUE_CAPACITY: usize = 64;

struct ControlProxySeedSource<'proxy, 'conn> {
    proxy: &'proxy ControlProxy<'conn>,
}

impl PopupSeedSource for ControlProxySeedSource<'_, '_> {
    async fn seed_snapshot(&self) -> Result<SeedSnapshot, SeedError> {
        // GetState is the owner handshake and must finish before snapshot calls begin
        let state = timed_dbus_call(self.proxy.get_state()).await;
        let state = match state {
            Ok(state) => state,
            Err(error) => {
                return SeedSnapshot::from_fetch_results(Err(error), Ok(Vec::new()));
            }
        };
        let active = timed_dbus_call(self.proxy.list_active()).await;
        let state = Ok(state);
        SeedSnapshot::from_fetch_results(state, active)
    }
}

pub fn start_dbus_runtime(sender: async_channel::Sender<UiEvent>) -> mpsc::Sender<UiCommand> {
    let (command_tx, command_rx) = mpsc::channel(UI_COMMAND_QUEUE_CAPACITY);
    spawn_runtime_thread(sender, command_rx);
    command_tx
}

fn spawn_runtime_thread(
    sender: async_channel::Sender<UiEvent>,
    command_rx: mpsc::Receiver<UiCommand>,
) {
    thread::spawn(move || {
        // Dedicated runtime keeps async D-Bus work off the GTK main thread
        let Some(runtime) = build_runtime() else {
            return;
        };
        runtime.block_on(run_dbus_loop(sender, command_rx));
    });
}

fn build_runtime() -> Option<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        // Small worker pool keeps background popups responsive without excess threads
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|err| {
            warn!(?err, "failed to initialize tokio runtime");
            err
        })
        .ok()
}

async fn run_dbus_loop(
    sender: async_channel::Sender<UiEvent>,
    mut command_rx: mpsc::Receiver<UiCommand>,
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
            Err(err) => {
                connect_log.warn_or_debug(&err, "failed to connect to session bus; retrying");
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
) -> Option<Duration> {
    let proxy = match ControlProxy::new(connection).await {
        Ok(proxy) => proxy,
        Err(err) => {
            subscribe_log.warn_or_debug(&err, "control interface unavailable, retrying");
            drain_offline_commands(command_rx);
            return Some(subscribe_backoff.next_sleep());
        }
    };
    let mut owner_changes = match proxy.inner().receive_owner_changed().await {
        Ok(stream) => stream,
        Err(error) => {
            subscribe_log.warn_or_debug(&error, "control owner watch unavailable, retrying");
            return Some(subscribe_backoff.next_sleep());
        }
    };
    let dbus = match DBusProxy::new(connection).await {
        Ok(proxy) => proxy,
        Err(error) => {
            subscribe_log.warn_or_debug(&error, "session bus owner proxy unavailable, retrying");
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
        match run_owner_generation(
            &proxy,
            &owner,
            &mut owner_changes,
            sender,
            command_rx,
            subscribe_backoff,
            subscribe_log,
        )
        .await
        {
            GenerationExit::OwnerChanged => {}
            GenerationExit::ConnectionLost => return Some(subscribe_backoff.next_sleep()),
            GenerationExit::Shutdown => return None,
            GenerationExit::Retry => {
                tokio::time::sleep(subscribe_backoff.next_sleep()).await;
            }
        }
    }
}

enum OwnerWait {
    Ready(String),
    Disconnected,
    Shutdown,
}

enum GenerationExit {
    OwnerChanged,
    ConnectionLost,
    Shutdown,
    Retry,
}

async fn wait_for_control_owner(
    dbus: &DBusProxy<'_>,
    owner_changes: &mut OwnerChangedStream<'_>,
    sender: &async_channel::Sender<UiEvent>,
    command_rx: &mut mpsc::Receiver<UiCommand>,
) -> OwnerWait {
    let control_name = BusName::try_from(CONTROL_BUS_NAME)
        .expect("static UnixNotis control bus name must be valid");
    if let Ok(Ok(owner)) = tokio::time::timeout(
        INTERNAL_DBUS_CALL_TIMEOUT,
        dbus.get_name_owner(control_name),
    )
    .await
    {
        return OwnerWait::Ready(owner.to_string());
    }

    // No owner is a quiet disconnected state until the broker announces one
    let _ = sender.send(UiEvent::Disconnected).await;
    drain_offline_commands(command_rx);
    loop {
        tokio::select! {
            command = command_rx.recv() => {
                if command.is_none() {
                    return OwnerWait::Shutdown;
                }
                warn!("dropping popup command while control service has no owner");
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

async fn run_owner_generation(
    proxy: &ControlProxy<'_>,
    owner: &str,
    owner_changes: &mut OwnerChangedStream<'_>,
    sender: &async_channel::Sender<UiEvent>,
    command_rx: &mut mpsc::Receiver<UiCommand>,
    subscribe_backoff: &mut Backoff,
    subscribe_log: &mut RetryLog,
) -> GenerationExit {
    // Popups stay on the shared notification stream, but the trimmed payload keeps
    // each message smaller now that unused flags were removed from NotificationView
    let mut added_stream = match proxy.receive_notification_added().await {
        Ok(stream) => stream,
        Err(err) => {
            subscribe_log.warn_or_debug(&err, "failed to subscribe to notification_added");
            return GenerationExit::Retry;
        }
    };
    let mut updated_stream = match proxy.receive_notification_updated().await {
        Ok(stream) => stream,
        Err(err) => {
            subscribe_log.warn_or_debug(&err, "failed to subscribe to notification_updated");
            return GenerationExit::Retry;
        }
    };
    let mut closed_stream = match proxy.receive_notification_closed().await {
        Ok(stream) => stream,
        Err(err) => {
            subscribe_log.warn_or_debug(&err, "failed to subscribe to notification_closed");
            return GenerationExit::Retry;
        }
    };
    let mut popup_gate_stream = match proxy.receive_popup_gate_changed().await {
        Ok(stream) => stream,
        Err(err) => {
            subscribe_log.warn_or_debug(&err, "failed to subscribe to popup_gate_changed");
            return GenerationExit::Retry;
        }
    };
    let mut invalidated_stream = match proxy.receive_snapshot_invalidated().await {
        Ok(stream) => stream,
        Err(err) => {
            subscribe_log.warn_or_debug(&err, "failed to subscribe to snapshot_invalidated");
            return GenerationExit::Retry;
        }
    };

    // Seed only after subscriptions are active so startup does not miss in-flight changes
    if let Err(error) = seed_state(&ControlProxySeedSource { proxy }, sender).await {
        subscribe_log.warn_or_debug(&error, "popup readiness handshake or seed failed");
        return GenerationExit::Retry;
    }
    subscribe_backoff.reset();
    subscribe_log.reset();
    info!(owner, "UnixNotis control service ready");

    let exit = loop {
        tokio::select! {
            command = command_rx.recv() => {
                let Some(command) = command else {
                    break GenerationExit::Shutdown;
                };
                if let Err(err) = handle_command(proxy, command).await {
                    warn!(?err, "control command failed");
                }
            }
            signal = added_stream.next() => {
                let Some(signal) = signal else {
                    warn!("notification_added stream ended");
                    break GenerationExit::OwnerChanged;
                };
                if let Ok(args) = signal.args() {
                    push_active_notification_event(
                        proxy,
                        sender,
                        *args.id(),
                        *args.show_popup(),
                        true,
                    ).await;
                }
            }
            signal = updated_stream.next() => {
                let Some(signal) = signal else {
                    warn!("notification_updated stream ended");
                    break GenerationExit::OwnerChanged;
                };
                if let Ok(args) = signal.args() {
                    push_active_notification_event(
                        proxy,
                        sender,
                        *args.id(),
                        *args.show_popup(),
                        false,
                    ).await;
                }
            }
            signal = closed_stream.next() => {
                let Some(signal) = signal else {
                    warn!("notification_closed stream ended");
                    break GenerationExit::OwnerChanged;
                };
                if let Ok(args) = signal.args() {
                    let _ = sender
                        .send(UiEvent::NotificationClosed(
                            *args.id(),
                            *args.reason(),
                        ))
                        .await;
                }
            }
            signal = popup_gate_stream.next() => {
                let Some(signal) = signal else {
                    warn!("popup_gate_changed stream ended");
                    break GenerationExit::OwnerChanged;
                };
                if let Ok(args) = signal.args() {
                    let _ = sender
                        .send(UiEvent::PopupGateChanged(args.gate().clone()))
                        .await;
                }
            }
            signal = invalidated_stream.next() => {
                let Some(_signal) = signal else {
                    warn!("snapshot_invalidated stream ended");
                    break GenerationExit::OwnerChanged;
                };
                // A fresh seed clears stale popups after remote clears or daemon restart drift
                // Seed reconcile also updates same-id payload changes without trusting missed signals
                if let Err(error) = seed_state(&ControlProxySeedSource { proxy }, sender).await {
                    subscribe_log.warn_or_debug(&error, "popup snapshot refresh failed");
                    break GenerationExit::Retry;
                }
            }
            owner_update = owner_changes.next() => {
                match owner_update {
                    Some(Some(new_owner)) => {
                        warn!(owner = new_owner.as_str(), "UnixNotis control owner changed");
                        let _ = sender.send(UiEvent::Disconnected).await;
                        break GenerationExit::OwnerChanged;
                    }
                    Some(None) => {
                        info!("UnixNotis control service disconnected");
                        let _ = sender.send(UiEvent::Disconnected).await;
                        break GenerationExit::OwnerChanged;
                    }
                    None => {
                        warn!("control owner stream ended");
                        let _ = sender.send(UiEvent::Disconnected).await;
                        break GenerationExit::ConnectionLost;
                    }
                }
            }
        }
    };

    exit
}

async fn push_active_notification_event(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<UiEvent>,
    id: u32,
    show_popup: bool,
    is_add: bool,
) {
    // Full popup payloads now stay on the authorized pull path instead of the shared signal
    match timed_dbus_call(proxy.get_active_notification(id)).await {
        Ok(mut notifications) => {
            // Close fanout can win the race, so a missing row is a normal no-op here
            let Some(notification) = notifications.pop() else {
                return;
            };
            let event = if is_add {
                UiEvent::NotificationAdded(notification, show_popup)
            } else {
                UiEvent::NotificationUpdated(notification, show_popup)
            };
            let _ = sender.send(event).await;
        }
        Err(err) => {
            warn!(?err, id, "failed to fetch popup notification after signal");
        }
    }
}

#[cfg(test)]
#[path = "tests/runtime.rs"]
mod tests;
