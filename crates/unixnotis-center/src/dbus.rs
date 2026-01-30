//! D-Bus runtime for center UI events and control commands.

// Submodules live in src/dbus/ to keep the control loop focused and readable.
mod dbus_backoff;
mod dbus_commands;
mod dbus_seed;
mod dbus_types;

use std::collections::VecDeque;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::mpsc;
use tracing::{info, warn};
use unixnotis_core::ControlProxy;
use zbus::Connection;

use dbus_backoff::{Backoff, RetryLog, BACKOFF_BASE_MS, BACKOFF_MAX_MS, RETRY_WARN_INTERVAL_SECS};
use dbus_commands::{
    drop_stale_offline_commands, flush_offline_commands, handle_command, stash_offline_commands,
};
use dbus_seed::seed_state_with_retry;

pub use dbus_types::{UiCommand, UiEvent};

// Bound UI command queue to prevent unbounded growth during stalls.
const UI_COMMAND_QUEUE_CAPACITY: usize = 64;

pub fn start_dbus_task(
    runtime: &tokio::runtime::Handle,
    connection: Connection,
    sender: async_channel::Sender<UiEvent>,
) -> mpsc::Sender<UiCommand> {
    let (command_tx, command_rx) = mpsc::channel(UI_COMMAND_QUEUE_CAPACITY);
    runtime.spawn(run_dbus_loop(connection, sender, command_rx));
    command_tx
}

async fn run_dbus_loop(
    connection: Connection,
    sender: async_channel::Sender<UiEvent>,
    mut command_rx: mpsc::Receiver<UiCommand>,
) {
    // Buffer UI actions during reconnect to avoid losing user intent.
    let mut offline_commands: VecDeque<UiCommand> = VecDeque::new();
    let mut connect_backoff = Backoff::new(BACKOFF_BASE_MS, BACKOFF_MAX_MS);
    let mut subscribe_backoff = Backoff::new(BACKOFF_BASE_MS, BACKOFF_MAX_MS);
    let mut connect_log = RetryLog::new(Duration::from_secs(RETRY_WARN_INTERVAL_SECS));
    let mut subscribe_log = RetryLog::new(Duration::from_secs(RETRY_WARN_INTERVAL_SECS));

    loop {
        let proxy = match ControlProxy::new(&connection).await {
            Ok(proxy) => proxy,
            Err(err) => {
                connect_log.warn_or_debug(&err, "control interface unavailable, retrying");
                stash_offline_commands(&mut command_rx, &mut offline_commands);
                tokio::time::sleep(connect_backoff.next_sleep()).await;
                continue;
            }
        };
        connect_backoff.reset();
        connect_log.reset();
        info!("connected to unixnotis control interface");

        // Subscribe to signal streams before seeding so match rules are installed
        // early and in-flight events are buffered while the seed request runs.
        let mut added_stream = match proxy.receive_notification_added().await {
            Ok(stream) => stream,
            Err(err) => {
                subscribe_log.warn_or_debug(&err, "failed to subscribe to notification_added");
                tokio::time::sleep(subscribe_backoff.next_sleep()).await;
                continue;
            }
        };
        let mut updated_stream = match proxy.receive_notification_updated().await {
            Ok(stream) => stream,
            Err(err) => {
                subscribe_log.warn_or_debug(&err, "failed to subscribe to notification_updated");
                tokio::time::sleep(subscribe_backoff.next_sleep()).await;
                continue;
            }
        };
        let mut closed_stream = match proxy.receive_notification_closed().await {
            Ok(stream) => stream,
            Err(err) => {
                subscribe_log.warn_or_debug(&err, "failed to subscribe to notification_closed");
                tokio::time::sleep(subscribe_backoff.next_sleep()).await;
                continue;
            }
        };
        let mut state_stream = match proxy.receive_state_changed().await {
            Ok(stream) => stream,
            Err(err) => {
                subscribe_log.warn_or_debug(&err, "failed to subscribe to state_changed");
                tokio::time::sleep(subscribe_backoff.next_sleep()).await;
                continue;
            }
        };
        let mut panel_stream = match proxy.receive_panel_requested().await {
            Ok(stream) => stream,
            Err(err) => {
                subscribe_log.warn_or_debug(&err, "failed to subscribe to panel_requested");
                tokio::time::sleep(subscribe_backoff.next_sleep()).await;
                continue;
            }
        };
        subscribe_backoff.reset();
        subscribe_log.reset();

        // Seed after subscriptions to avoid dropping events that arrive during the
        // initial state fetch. The streams buffer messages until polling resumes.
        seed_state_with_retry(&proxy, &sender).await;
        drop_stale_offline_commands(&mut offline_commands);
        flush_offline_commands(&proxy, &sender, &mut offline_commands).await;

        loop {
            tokio::select! {
                command = command_rx.recv() => {
                    let Some(command) = command else {
                        break;
                    };
                    if let Err(err) = handle_command(&proxy, &sender, command).await {
                        warn!(?err, "control command failed");
                    }
                }
                signal = added_stream.next() => {
                    let Some(signal) = signal else {
                        warn!("notification_added stream ended");
                        break;
                    };
                    if let Ok(args) = signal.args() {
                        let _ = sender
                            .send(UiEvent::NotificationAdded(
                                args.notification().clone(),
                                *args.show_popup(),
                            ))
                            .await;
                    }
                }
                signal = updated_stream.next() => {
                    let Some(signal) = signal else {
                        warn!("notification_updated stream ended");
                        break;
                    };
                    if let Ok(args) = signal.args() {
                        let _ = sender
                            .send(UiEvent::NotificationUpdated(
                                args.notification().clone(),
                                *args.show_popup(),
                            ))
                            .await;
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
                signal = state_stream.next() => {
                    let Some(signal) = signal else {
                        warn!("state_changed stream ended");
                        break;
                    };
                    if let Ok(args) = signal.args() {
                        let _ = sender.send(UiEvent::StateChanged(args.state().clone())).await;
                    }
                }
                signal = panel_stream.next() => {
                    let Some(signal) = signal else {
                        warn!("panel_requested stream ended");
                        break;
                    };
                    if let Ok(args) = signal.args() {
                        let _ = sender.send(UiEvent::PanelRequested(*args.request())).await;
                    }
                }
            }
        }
        stash_offline_commands(&mut command_rx, &mut offline_commands);
        tokio::time::sleep(subscribe_backoff.next_sleep()).await;
    }
}
