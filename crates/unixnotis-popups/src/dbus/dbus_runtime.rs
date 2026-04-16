//! Popup D-Bus runtime bootstrap and stream loop

use std::thread;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::mpsc;
use tracing::{info, warn};
use unixnotis_core::ControlProxy;
use zbus::Connection;

use super::dbus_backoff::{
    Backoff, RetryLog, BACKOFF_BASE_MS, BACKOFF_MAX_MS, RETRY_WARN_INTERVAL_SECS,
};
use super::dbus_commands::{drain_offline_commands, handle_command};
use super::dbus_seed::seed_state_with_retry;
use super::dbus_types::{UiCommand, UiEvent};

// Bound UI commands to avoid unbounded memory growth under a stuck UI event loop
const UI_COMMAND_QUEUE_CAPACITY: usize = 64;

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
        let retry_delay = run_connection_once(
            &connection,
            &sender,
            &mut command_rx,
            &mut subscribe_backoff,
            &mut subscribe_log,
        )
        .await;
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
) -> Duration {
    let proxy = match ControlProxy::new(connection).await {
        Ok(proxy) => proxy,
        Err(err) => {
            subscribe_log.warn_or_debug(&err, "control interface unavailable, retrying");
            drain_offline_commands(command_rx);
            return subscribe_backoff.next_sleep();
        }
    };
    subscribe_backoff.reset();
    subscribe_log.reset();
    info!("connected to unixnotis control interface");

    // Popups stay on the shared notification stream, but the trimmed payload keeps
    // each message smaller now that unused flags were removed from NotificationView
    let mut added_stream = match proxy.receive_notification_added().await {
        Ok(stream) => stream,
        Err(err) => {
            subscribe_log.warn_or_debug(&err, "failed to subscribe to notification_added");
            return subscribe_backoff.next_sleep();
        }
    };
    let mut updated_stream = match proxy.receive_notification_updated().await {
        Ok(stream) => stream,
        Err(err) => {
            subscribe_log.warn_or_debug(&err, "failed to subscribe to notification_updated");
            return subscribe_backoff.next_sleep();
        }
    };
    let mut closed_stream = match proxy.receive_notification_closed().await {
        Ok(stream) => stream,
        Err(err) => {
            subscribe_log.warn_or_debug(&err, "failed to subscribe to notification_closed");
            return subscribe_backoff.next_sleep();
        }
    };
    let mut popup_gate_stream = match proxy.receive_popup_gate_changed().await {
        Ok(stream) => stream,
        Err(err) => {
            subscribe_log.warn_or_debug(&err, "failed to subscribe to popup_gate_changed");
            return subscribe_backoff.next_sleep();
        }
    };
    let mut invalidated_stream = match proxy.receive_snapshot_invalidated().await {
        Ok(stream) => stream,
        Err(err) => {
            subscribe_log.warn_or_debug(&err, "failed to subscribe to snapshot_invalidated");
            return subscribe_backoff.next_sleep();
        }
    };

    // Seed only after subscriptions are active so startup does not miss in-flight changes
    seed_state_with_retry(&proxy, sender).await;

    loop {
        tokio::select! {
            command = command_rx.recv() => {
                let Some(command) = command else {
                    break;
                };
                if let Err(err) = handle_command(&proxy, command).await {
                    warn!(?err, "control command failed");
                }
            }
            signal = added_stream.next() => {
                let Some(signal) = signal else {
                    warn!("notification_added stream ended");
                    break;
                };
                if let Ok(args) = signal.args() {
                    push_active_notification_event(
                        &proxy,
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
                    break;
                };
                if let Ok(args) = signal.args() {
                    push_active_notification_event(
                        &proxy,
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
                    break;
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
                    break;
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
                    break;
                };
                // A fresh seed clears stale popups after remote clears or daemon restart drift
                // Seed reconcile also updates same-id payload changes without trusting missed signals
                seed_state_with_retry(&proxy, sender).await;
            }
        }
    }

    subscribe_backoff.next_sleep()
}

async fn push_active_notification_event(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<UiEvent>,
    id: u32,
    show_popup: bool,
    is_add: bool,
) {
    // Full popup payloads now stay on the authorized pull path instead of the shared signal
    match proxy.get_active_notification(id).await {
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
