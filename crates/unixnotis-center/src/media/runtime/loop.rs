use futures_util::StreamExt;
use tokio::sync::mpsc;
use tracing::warn;
use unixnotis_core::reconnect::{
    Backoff, RetryLog, BACKOFF_BASE_MS, BACKOFF_MAX_MS, RETRY_WARN_INTERVAL_SECS,
};
use unixnotis_core::MediaConfig;
use zbus::fdo::DBusProxy;
use zbus::Connection;

use crate::control::UiEvent;

use super::super::events::{
    apply_owner_change, handle_runtime_command, handle_runtime_signal, refresh_all_players,
    OwnerChangeOutcome,
};
use super::schedule::DelayedRefreshTasks;
use super::{MediaSignal, MEDIA_SIGNAL_CAPACITY};
use crate::media::mpris::PlayerState;
use crate::media::{MediaCommand, MediaInfo};

const OWNER_REBUILD_RETRY_MS: u64 = 200;

pub(in crate::media) struct MediaRuntimeState {
    // Live player proxies keyed by bus name
    pub(in crate::media) players: std::collections::HashMap<String, PlayerState>,
    // Last known media snapshot per player
    pub(in crate::media) cache: std::collections::HashMap<String, MediaInfo>,
    // Last emitted snapshot lets the loop drop duplicate UI updates cheaply
    pub(in crate::media) last_snapshot: Vec<MediaInfo>,
    // One delayed retry plan per player
    pub(in crate::media) delayed_refreshes: DelayedRefreshTasks,
}

impl MediaRuntimeState {
    fn new() -> Self {
        // A fresh loop starts empty and fills from the first refresh pass
        Self {
            players: std::collections::HashMap::new(),
            cache: std::collections::HashMap::new(),
            last_snapshot: Vec::new(),
            delayed_refreshes: std::collections::HashMap::new(),
        }
    }
}

pub(super) async fn run_event_loop(
    config: MediaConfig,
    sender: async_channel::Sender<UiEvent>,
    mut command_rx: mpsc::Receiver<MediaCommand>,
) {
    let mut backoff = Backoff::new(BACKOFF_BASE_MS, BACKOFF_MAX_MS);
    let mut retry_log = RetryLog::new(std::time::Duration::from_secs(RETRY_WARN_INTERVAL_SECS));

    loop {
        // Player commands contain names from one bus generation and cannot cross reconnects
        drain_stale_media_commands(&mut command_rx);
        if command_rx.is_closed() {
            return;
        }
        let connection = match Connection::session().await {
            Ok(connection) => connection,
            Err(err) => {
                retry_log.warn_or_debug(&err, "failed to connect media runtime to session bus");
                let _ = sender.send(UiEvent::MediaCleared).await;
                tokio::time::sleep(backoff.next_sleep()).await;
                continue;
            }
        };
        backoff.reset();
        retry_log.reset();

        if run_connection_once(&connection, &config, &sender, &mut command_rx).await {
            return;
        }

        // Dropping the session clears every stale proxy before another generation is accepted
        let _ = sender.send(UiEvent::MediaCleared).await;
        tokio::time::sleep(backoff.next_sleep()).await;
    }
}

// Returns true only when the command sender closed and the runtime should stop
async fn run_connection_once(
    connection: &Connection,
    config: &MediaConfig,
    sender: &async_channel::Sender<UiEvent>,
    command_rx: &mut mpsc::Receiver<MediaCommand>,
) -> bool {
    let dbus_proxy = match DBusProxy::new(connection).await {
        Ok(proxy) => proxy,
        Err(err) => {
            warn!(?err, "failed to create D-Bus proxy for media");
            return false;
        }
    };

    let mut owner_stream = match dbus_proxy.receive_name_owner_changed().await {
        Ok(stream) => stream,
        Err(err) => {
            warn!(?err, "failed to subscribe to name owner changes");
            return false;
        }
    };

    // This channel keeps property updates away from the GTK thread
    let (signal_tx, mut signal_rx) = mpsc::channel::<MediaSignal>(MEDIA_SIGNAL_CAPACITY);
    let (owner_retry_tx, mut owner_retry_rx) = mpsc::channel::<()>(1);
    let mut state = MediaRuntimeState::new();
    // Startup begins with one full refresh so the UI gets a complete snapshot
    let mut refresh = true;
    let mut owner_retry_scheduled = false;

    loop {
        if refresh {
            refresh_all_players(
                connection,
                &dbus_proxy,
                config,
                &signal_tx,
                &mut state,
                sender,
            )
            .await;
            refresh = false;
        }

        tokio::select! {
            command = command_rx.recv() => {
                let Some(command) = command else {
                    // Closing the command side shuts the media runtime down cleanly
                    return true;
                };
                match command {
                    MediaCommand::Refresh => {
                        refresh = true;
                    }
                    command => {
                        handle_runtime_command(&mut state, &signal_tx, sender, command).await;
                    }
                }
            }
            signal = signal_rx.recv() => {
                let Some(signal) = signal else {
                    // Property listeners belong to this connection and must be rebuilt together
                    return false;
                };
                handle_runtime_signal(&mut state, &signal_tx, sender, signal).await;
            }
            retry = owner_retry_rx.recv() => {
                if retry.is_none() {
                    return false;
                }
                // One delayed discovery pass repairs a transient owner-proxy construction failure
                owner_retry_scheduled = false;
                refresh = true;
            }
            signal = owner_stream.next() => {
                let Some(signal) = signal else {
                    // Stream termination means zbus can no longer deliver this bus generation
                    return false;
                };
                if let Ok(args) = signal.args() {
                    let name = args.name();
                    let new_owner = args
                        .new_owner()
                        .as_ref()
                        .map(|owner| owner.as_str().to_string());
                    let owner_change = apply_owner_change(
                        name,
                        new_owner.as_deref(),
                        connection,
                        config,
                        &signal_tx,
                        &mut state,
                        sender,
                    )
                    .await;
                    let retry_needed = match owner_change {
                        Ok(outcome) => owner_change_needs_retry(outcome),
                        Err(err) => {
                            warn!(?err, "failed to apply media owner change");
                            true
                        }
                    };
                    if retry_needed && !owner_retry_scheduled {
                        // Only one timer may exist so repeated owner noise cannot become polling
                        owner_retry_scheduled = true;
                        tokio::spawn(send_owner_rebuild_retry_after(
                            std::time::Duration::from_millis(OWNER_REBUILD_RETRY_MS),
                            owner_retry_tx.clone(),
                        ));
                    }
                }
            }
        }
    }
}

const fn owner_change_needs_retry(outcome: OwnerChangeOutcome) -> bool {
    matches!(outcome, OwnerChangeOutcome::RetryNeeded)
}

async fn send_owner_rebuild_retry_after(delay: std::time::Duration, sender: mpsc::Sender<()>) {
    tokio::time::sleep(delay).await;
    // A closed receiver means the bus generation already ended and no retry remains useful
    let _ = sender.send(()).await;
}

pub(super) fn drain_stale_media_commands(command_rx: &mut mpsc::Receiver<MediaCommand>) {
    while command_rx.try_recv().is_ok() {
        // Startup refreshes all players, so queued commands from the dead bus are obsolete
    }
}

#[cfg(test)]
#[path = "../tests/event_loop.rs"]
mod tests;
