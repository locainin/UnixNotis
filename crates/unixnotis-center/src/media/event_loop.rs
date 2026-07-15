use futures_util::StreamExt;
use tokio::sync::mpsc;
use tracing::warn;
use unixnotis_core::reconnect::{
    Backoff, RetryLog, BACKOFF_BASE_MS, BACKOFF_MAX_MS, RETRY_WARN_INTERVAL_SECS,
};
use unixnotis_core::MediaConfig;
use zbus::fdo::DBusProxy;
use zbus::Connection;

use crate::dbus::UiEvent;

use super::bus::PlayerState;
use super::events::{
    apply_owner_change, handle_runtime_command, handle_runtime_signal, refresh_all_players,
};
use super::runtime::MEDIA_SIGNAL_CAPACITY;
use super::schedule::DelayedRefreshTasks;
use super::{MediaCommand, MediaInfo, MediaSignal};

pub(super) struct MediaRuntimeState {
    // Live player proxies keyed by bus name
    pub(super) players: std::collections::HashMap<String, PlayerState>,
    // Last known media snapshot per player
    pub(super) cache: std::collections::HashMap<String, MediaInfo>,
    // Last emitted snapshot lets the loop drop duplicate UI updates cheaply
    pub(super) last_snapshot: Vec<MediaInfo>,
    // One delayed retry plan per player
    pub(super) delayed_refreshes: DelayedRefreshTasks,
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
    let mut state = MediaRuntimeState::new();
    // Startup begins with one full refresh so the UI gets a complete snapshot
    let mut refresh = true;

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
                    if let Err(err) = apply_owner_change(
                        name,
                        new_owner.as_deref(),
                        connection,
                        config,
                        &signal_tx,
                        &mut state,
                        sender,
                    )
                    .await
                    {
                        warn!(?err, "failed to apply media owner change");
                    }
                }
            }
        }
    }
}

pub(super) fn drain_stale_media_commands(command_rx: &mut mpsc::Receiver<MediaCommand>) {
    while command_rx.try_recv().is_ok() {
        // Startup refreshes all players, so queued commands from the dead bus are obsolete
    }
}
