//! One control-owner generation from subscription through orderly cleanup

use futures_util::StreamExt;
use tokio::sync::{mpsc, watch};
use tracing::{info, warn};
use unixnotis_core::{
    timed_dbus_call, ControlProxy, NotificationAddedStream, NotificationClosedStream,
    NotificationUpdatedStream, PopupGateChangedStream, SnapshotInvalidatedStream,
};
use zbus::proxy::OwnerChangedStream;

use super::delivery::push_active_notification_event;
use super::readiness::{wait_for_gtk_runtime, PopupReadinessLease};
use crate::dbus::backoff::{Backoff, RetryLog};
use crate::dbus::commands::handle_command;
use crate::dbus::seed::{seed_state, PopupSeedSource, SeedError, SeedSnapshot};
use crate::dbus::{UiCommand, UiEvent};

struct ControlProxySeedSource<'proxy, 'connection> {
    proxy: &'proxy ControlProxy<'connection>,
}

impl PopupSeedSource for ControlProxySeedSource<'_, '_> {
    async fn seed_snapshot(&self) -> Result<SeedSnapshot, SeedError> {
        // GetState proves the owner can serve the expected control interface
        let state = match timed_dbus_call(self.proxy.get_state()).await {
            Ok(state) => state,
            Err(error) => {
                return SeedSnapshot::from_fetch_results(Err(error), Ok(Vec::new()));
            }
        };
        let active = timed_dbus_call(self.proxy.list_popup_candidates()).await;
        SeedSnapshot::from_fetch_results(Ok(state), active)
    }
}

pub(super) enum GenerationExit {
    OwnerChanged,
    ConnectionLost,
    Shutdown,
    Retry,
}

pub(super) struct PopupGenerationContext<'context, 'stream> {
    owner_changes: &'context mut OwnerChangedStream<'stream>,
    sender: &'context async_channel::Sender<UiEvent>,
    command_rx: &'context mut mpsc::Receiver<UiCommand>,
    subscribe_backoff: &'context mut Backoff,
    subscribe_log: &'context mut RetryLog,
    gtk_ready_rx: &'context mut watch::Receiver<bool>,
}

impl<'context, 'stream> PopupGenerationContext<'context, 'stream> {
    pub(super) const fn new(
        owner_changes: &'context mut OwnerChangedStream<'stream>,
        sender: &'context async_channel::Sender<UiEvent>,
        command_rx: &'context mut mpsc::Receiver<UiCommand>,
        subscribe_backoff: &'context mut Backoff,
        subscribe_log: &'context mut RetryLog,
        gtk_ready_rx: &'context mut watch::Receiver<bool>,
    ) -> Self {
        Self {
            owner_changes,
            sender,
            command_rx,
            subscribe_backoff,
            subscribe_log,
            gtk_ready_rx,
        }
    }
}

struct GenerationStreams<'proxy> {
    added: NotificationAddedStream<'proxy>,
    updated: NotificationUpdatedStream<'proxy>,
    closed: NotificationClosedStream<'proxy>,
    gate: PopupGateChangedStream<'proxy>,
    invalidated: SnapshotInvalidatedStream<'proxy>,
}

struct SubscribeError {
    signal: &'static str,
    source: zbus::Error,
}

impl GenerationStreams<'_> {
    async fn subscribe<'proxy>(
        proxy: &'proxy ControlProxy<'_>,
    ) -> Result<GenerationStreams<'proxy>, SubscribeError> {
        let added = proxy
            .receive_notification_added()
            .await
            .map_err(|source| SubscribeError {
                signal: "notification_added",
                source,
            })?;
        let updated = proxy
            .receive_notification_updated()
            .await
            .map_err(|source| SubscribeError {
                signal: "notification_updated",
                source,
            })?;
        let closed = proxy
            .receive_notification_closed()
            .await
            .map_err(|source| SubscribeError {
                signal: "notification_closed",
                source,
            })?;
        let gate = proxy
            .receive_popup_gate_changed()
            .await
            .map_err(|source| SubscribeError {
                signal: "popup_gate_changed",
                source,
            })?;
        let invalidated = proxy
            .receive_snapshot_invalidated()
            .await
            .map_err(|source| SubscribeError {
                signal: "snapshot_invalidated",
                source,
            })?;
        Ok(GenerationStreams {
            added,
            updated,
            closed,
            gate,
            invalidated,
        })
    }
}

pub(super) async fn run_owner_generation(
    proxy: &ControlProxy<'_>,
    owner: &str,
    context: PopupGenerationContext<'_, '_>,
) -> GenerationExit {
    let PopupGenerationContext {
        owner_changes,
        sender,
        command_rx,
        subscribe_backoff,
        subscribe_log,
        gtk_ready_rx,
    } = context;
    let mut streams = match GenerationStreams::subscribe(proxy).await {
        Ok(streams) => streams,
        Err(error) => {
            subscribe_log.warn_or_debug(
                &error.source,
                &format!("failed to subscribe to {}", error.signal),
            );
            return GenerationExit::Retry;
        }
    };

    // Subscription precedes the seed so no change can fall between both phases
    if let Err(error) = seed_state(&ControlProxySeedSource { proxy }, sender).await {
        subscribe_log.warn_or_debug(&error, "popup readiness handshake or seed failed");
        return GenerationExit::Retry;
    }
    if !wait_for_gtk_runtime(gtk_ready_rx).await {
        warn!("popup GTK runtime did not become ready");
        return GenerationExit::Retry;
    }
    let mut readiness = PopupReadinessLease::new(proxy);
    if let Err(error) = readiness.publish().await {
        subscribe_log.warn_or_debug(&error, "failed to mark popup renderer ready");
        return GenerationExit::Retry;
    }
    subscribe_backoff.reset();
    subscribe_log.reset();
    info!(owner, "UnixNotis control service ready");

    let mut shutdown_acknowledgement = None;
    let exit = loop {
        tokio::select! {
            command = command_rx.recv() => {
                let Some(command) = command else {
                    break GenerationExit::Shutdown;
                };
                match command {
                    UiCommand::Shutdown(acknowledgement) => {
                        shutdown_acknowledgement = Some(acknowledgement);
                        break GenerationExit::Shutdown;
                    }
                    command => {
                        if let Err(error) = handle_command(proxy, command).await {
                            warn!(?error, "popup control command failed");
                        }
                    }
                }
            }
            signal = streams.added.next() => {
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
            signal = streams.updated.next() => {
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
            signal = streams.closed.next() => {
                let Some(signal) = signal else {
                    warn!("notification_closed stream ended");
                    break GenerationExit::OwnerChanged;
                };
                if let Ok(args) = signal.args() {
                    let _ = sender
                        .send(UiEvent::NotificationClosed(*args.id(), *args.reason()))
                        .await;
                }
            }
            signal = streams.gate.next() => {
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
            signal = streams.invalidated.next() => {
                let Some(_signal) = signal else {
                    warn!("snapshot_invalidated stream ended");
                    break GenerationExit::OwnerChanged;
                };
                // Re-seeding reconciles missed updates without publishing a second readiness lease
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

    readiness.clear().await;
    if let Some(acknowledgement) = shutdown_acknowledgement {
        let _ = acknowledgement.send(());
    }
    exit
}
