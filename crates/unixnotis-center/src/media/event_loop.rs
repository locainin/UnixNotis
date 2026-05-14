use futures_util::StreamExt;
use tokio::sync::mpsc;
use tracing::warn;
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
    connection: Connection,
    config: MediaConfig,
    sender: async_channel::Sender<UiEvent>,
    mut command_rx: mpsc::Receiver<MediaCommand>,
) {
    let dbus_proxy = match DBusProxy::new(&connection).await {
        Ok(proxy) => proxy,
        Err(err) => {
            warn!(?err, "failed to create D-Bus proxy for media");
            return;
        }
    };

    let mut owner_stream = match dbus_proxy.receive_name_owner_changed().await {
        Ok(stream) => stream,
        Err(err) => {
            warn!(?err, "failed to subscribe to name owner changes");
            return;
        }
    };

    // This channel keeps property updates away from the GTK thread
    let (signal_tx, mut signal_rx) = mpsc::channel::<MediaSignal>(MEDIA_SIGNAL_CAPACITY);
    let mut state = MediaRuntimeState::new();
    // Startup begins with one full refresh so the UI gets a complete snapshot
    let mut refresh = true;

    loop {
        if refresh {
            // Full refresh rebuilds the visible player set from the bus
            refresh_all_players(
                &connection,
                &dbus_proxy,
                &config,
                &signal_tx,
                &mut state,
                &sender,
            )
            .await;
            refresh = false;
        }

        tokio::select! {
            command = command_rx.recv() => {
                let Some(command) = command else {
                    // Closing the command side shuts the media runtime down cleanly
                    break;
                };
                match command {
                    MediaCommand::Refresh => {
                        // Full refresh is used after startup and explicit reloads
                        refresh = true;
                    }
                    command => {
                        handle_runtime_command(
                            &mut state,
                            &signal_tx,
                            &sender,
                            command,
                        ).await;
                    }
                }
            }
            signal = signal_rx.recv() => {
                let Some(signal) = signal else {
                    // A closed signal channel means no more property updates can arrive
                    break;
                };
                handle_runtime_signal(
                    &mut state,
                    &signal_tx,
                    &sender,
                    signal,
                ).await;
            }
            signal = owner_stream.next() => {
                let Some(signal) = signal else {
                    // If the owner stream ends, the bus subscription is gone too
                    break;
                };
                if let Ok(args) = signal.args() {
                    // Name owner changes tell the loop when players appear or vanish
                    let name = args.name();
                    let new_owner = args
                        .new_owner()
                        .as_ref()
                        .map(|owner| owner.as_str().to_string());
                    if let Err(err) = apply_owner_change(
                        name,
                        new_owner.as_deref(),
                        &connection,
                        &config,
                        &signal_tx,
                        &mut state,
                        &sender,
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
