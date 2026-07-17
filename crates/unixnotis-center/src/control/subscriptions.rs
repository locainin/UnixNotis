//! Control signal subscription setup and one connected event generation

use std::collections::VecDeque;

use futures_util::StreamExt;
use tokio::sync::mpsc;
use tracing::warn;
use unixnotis_core::ControlProxy;

use super::backoff::{Backoff, RetryLog};
use super::commands::{drop_stale_offline_commands, flush_offline_commands, handle_command};
use super::events::push_active_notification_event;
use super::model::{UiCommand, UiEvent};
use super::seed::seed_state_with_retry;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ControlGenerationExit {
    // Subscription setup already applied its bounded retry delay
    RetryDelayed,
    // A live stream ended and the reconnect owner must perform cleanup
    Disconnected,
    // The UI dropped every command sender and no longer needs a control task
    Shutdown,
}

impl ControlGenerationExit {
    pub(super) const fn requires_reconnect_cleanup(self) -> bool {
        matches!(self, Self::Disconnected)
    }

    pub(super) const fn should_stop(self) -> bool {
        matches!(self, Self::Shutdown)
    }
}

pub(super) async fn run_control_generation(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<UiEvent>,
    command_rx: &mut mpsc::Receiver<UiCommand>,
    offline_commands: &mut VecDeque<UiCommand>,
    subscribe_backoff: &mut Backoff,
    subscribe_log: &mut RetryLog,
) -> ControlGenerationExit {
    // Every stream below belongs to the same verified proxy generation
    // Install every match rule before seeding so in-flight signals remain buffered
    let mut added_stream = match proxy.receive_notification_added().await {
        Ok(stream) => stream,
        Err(err) => {
            subscribe_log.warn_or_debug(&err, "failed to subscribe to notification_added");
            retry_subscription(subscribe_backoff).await;
            return ControlGenerationExit::RetryDelayed;
        }
    };
    let mut updated_stream = match proxy.receive_notification_updated().await {
        Ok(stream) => stream,
        Err(err) => {
            subscribe_log.warn_or_debug(&err, "failed to subscribe to notification_updated");
            retry_subscription(subscribe_backoff).await;
            return ControlGenerationExit::RetryDelayed;
        }
    };
    let mut closed_stream = match proxy.receive_notification_closed().await {
        Ok(stream) => stream,
        Err(err) => {
            subscribe_log.warn_or_debug(&err, "failed to subscribe to notification_closed");
            retry_subscription(subscribe_backoff).await;
            return ControlGenerationExit::RetryDelayed;
        }
    };
    let mut state_stream = match proxy.receive_state_changed().await {
        Ok(stream) => stream,
        Err(err) => {
            subscribe_log.warn_or_debug(&err, "failed to subscribe to state_changed");
            retry_subscription(subscribe_backoff).await;
            return ControlGenerationExit::RetryDelayed;
        }
    };
    let mut invalidated_stream = match proxy.receive_snapshot_invalidated().await {
        Ok(stream) => stream,
        Err(err) => {
            subscribe_log.warn_or_debug(&err, "failed to subscribe to snapshot_invalidated");
            retry_subscription(subscribe_backoff).await;
            return ControlGenerationExit::RetryDelayed;
        }
    };
    let mut panel_stream = match proxy.receive_panel_requested().await {
        Ok(stream) => stream,
        Err(err) => {
            subscribe_log.warn_or_debug(&err, "failed to subscribe to panel_requested");
            retry_subscription(subscribe_backoff).await;
            return ControlGenerationExit::RetryDelayed;
        }
    };
    // All match rules are live before retry history is cleared
    subscribe_backoff.reset();
    subscribe_log.reset();

    // Seed after subscription so events arriving during the fetch wait in their streams
    seed_state_with_retry(proxy, sender).await;
    drop_stale_offline_commands(offline_commands);
    flush_offline_commands(proxy, sender, offline_commands).await;
    // Readiness is published only after initial state and buffered commands settle
    if let Err(err) = proxy.mark_panel_ready().await {
        subscribe_log.warn_or_debug(&err, "failed to mark panel ready");
        retry_subscription(subscribe_backoff).await;
        return ControlGenerationExit::RetryDelayed;
    }

    // A single select loop preserves signal order within each D-Bus stream
    let exit = loop {
        tokio::select! {
            command = command_rx.recv() => {
                // Closing the final UI sender retires this generation immediately
                let Some(command) = command else {
                    break ControlGenerationExit::Shutdown;
                };
                if let Err(err) = handle_command(proxy, sender, command).await {
                    warn!(?err, "control command failed");
                }
            }
            signal = added_stream.next() => {
                // Fetch the complete row after receiving the lightweight identifier
                let Some(signal) = signal else {
                    warn!("notification_added stream ended");
                    break ControlGenerationExit::Disconnected;
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
                    break ControlGenerationExit::Disconnected;
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
                    break ControlGenerationExit::Disconnected;
                };
                if let Ok(args) = signal.args() {
                    let _ = sender
                        .send(UiEvent::NotificationClosed(*args.id(), *args.reason()))
                        .await;
                }
            }
            signal = state_stream.next() => {
                let Some(signal) = signal else {
                    warn!("state_changed stream ended");
                    break ControlGenerationExit::Disconnected;
                };
                if let Ok(args) = signal.args() {
                    let _ = sender.send(UiEvent::StateChanged(args.state().clone())).await;
                }
            }
            signal = invalidated_stream.next() => {
                let Some(_signal) = signal else {
                    warn!("snapshot_invalidated stream ended");
                    break ControlGenerationExit::Disconnected;
                };
                // A full seed is required because another client may have deleted any row
                seed_state_with_retry(proxy, sender).await;
            }
            signal = panel_stream.next() => {
                let Some(signal) = signal else {
                    warn!("panel_requested stream ended");
                    break ControlGenerationExit::Disconnected;
                };
                if let Ok(args) = signal.args() {
                    let _ = sender.send(UiEvent::PanelRequested(*args.request())).await;
                }
            }
        }
    };

    // Readiness is best effort because a closed transport cannot accept cleanup calls
    let _ = proxy.mark_panel_not_ready().await;
    exit
}

async fn retry_subscription(backoff: &mut Backoff) {
    // Setup failures consume their delay here so the outer loop does not sleep twice
    tokio::time::sleep(backoff.next_sleep()).await;
}

#[cfg(test)]
#[path = "tests/subscriptions.rs"]
mod tests;
