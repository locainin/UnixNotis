//! Media runtime orchestration for the notification center.
//!
//! Keeps the runtime loop here while delegating focused helpers to media_* modules.

mod media_bus;
mod media_cache;
mod media_metadata;
mod media_schedule;

use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::mpsc::{self, UnboundedSender};
use tracing::warn;
use unixnotis_core::MediaConfig;
use zbus::fdo::DBusProxy;
use zbus::Connection;

use crate::dbus::UiEvent;

use media_bus::{
    build_player_state, handle_command, is_allowed_player, refresh_players,
    spawn_properties_listener, PlayerState,
};
use media_cache::{refresh_cache, refresh_player_cache, send_snapshot};
use media_schedule::{
    schedule_delayed_refresh, schedule_metadata_fallback, schedule_metadata_fallbacks,
};

// MPRIS base identifiers used to discover players on the session bus.
const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
const MPRIS_PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const MPRIS_APP: &str = "org.mpris.MediaPlayer2";

#[derive(Debug, Clone)]
pub struct MediaInfo {
    pub bus_name: String,
    pub identity: String,
    pub title: String,
    pub artist: String,
    pub playback_status: String,
    pub art_uri: Option<String>,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_next: bool,
    pub can_prev: bool,
}

#[derive(Debug, Clone)]
pub enum MediaCommand {
    Refresh,
    PlayPause { bus_name: String },
    Next { bus_name: String },
    Previous { bus_name: String },
}

#[derive(Debug)]
enum MediaSignal {
    PropertiesChanged(String),
}

#[derive(Clone)]
pub struct MediaHandle {
    command_tx: Option<UnboundedSender<MediaCommand>>,
}

impl MediaHandle {
    pub fn refresh(&self) {
        if let Some(tx) = &self.command_tx {
            let _ = tx.send(MediaCommand::Refresh);
        }
    }

    pub fn play_pause(&self, bus_name: &str) {
        if let Some(tx) = &self.command_tx {
            let _ = tx.send(MediaCommand::PlayPause {
                bus_name: bus_name.to_string(),
            });
        }
    }

    pub fn next(&self, bus_name: &str) {
        if let Some(tx) = &self.command_tx {
            let _ = tx.send(MediaCommand::Next {
                bus_name: bus_name.to_string(),
            });
        }
    }

    pub fn previous(&self, bus_name: &str) {
        if let Some(tx) = &self.command_tx {
            let _ = tx.send(MediaCommand::Previous {
                bus_name: bus_name.to_string(),
            });
        }
    }
}

pub fn start_media_runtime(
    config: MediaConfig,
    sender: async_channel::Sender<UiEvent>,
) -> Option<MediaHandle> {
    if !config.enabled {
        return None;
    }

    let mut config = config;
    // Normalize allow/deny lists once to avoid repeated lowercasing in hot paths.
    config.allowlist = config
        .allowlist
        .into_iter()
        .map(|entry| entry.to_lowercase())
        .collect();
    config.denylist = config
        .denylist
        .into_iter()
        .map(|entry| entry.to_lowercase())
        .collect();

    let (command_tx, mut command_rx) = mpsc::unbounded_channel();

    thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(err) => {
                warn!(?err, "failed to initialize media runtime");
                return;
            }
        };
        runtime.block_on(async move {
            let connection = match Connection::session().await {
                Ok(connection) => connection,
                Err(err) => {
                    warn!(?err, "failed to connect to session bus for media");
                    return;
                }
            };

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

            // Dedicated signal channel keeps property updates out of the UI thread.
            let (signal_tx, mut signal_rx) = mpsc::unbounded_channel::<MediaSignal>();
            let mut players: HashMap<String, PlayerState> = HashMap::new();
            let mut cache: HashMap<String, MediaInfo> = HashMap::new();
            let mut refresh = true;

            loop {
                if refresh {
                    if let Err(err) =
                        refresh_players(&connection, &dbus_proxy, &config, &signal_tx, &mut players)
                            .await
                    {
                        warn!(?err, "failed to refresh media players");
                    }
                    refresh_cache(&players, &mut cache).await;
                    send_snapshot(&sender, &cache).await;
                    schedule_metadata_fallbacks(&cache, signal_tx.clone());
                    refresh = false;
                }

                tokio::select! {
                    command = command_rx.recv() => {
                        let Some(command) = command else {
                            break;
                        };
                        match command {
                            MediaCommand::Refresh => {
                                refresh = true;
                            }
                            command => {
                                if let Ok(Some(name)) = handle_command(&players, command).await {
                                    // Post-command refresh keeps controls responsive without polling.
                                    refresh_player_cache(&players, &mut cache, &name).await;
                                    send_snapshot(&sender, &cache).await;
                                    schedule_metadata_fallback(&cache, signal_tx.clone(), &name);
                                    for delay_ms in [150_u64, 650_u64] {
                                        schedule_delayed_refresh(
                                            signal_tx.clone(),
                                            name.clone(),
                                            Duration::from_millis(delay_ms),
                                        );
                                    }
                                }
                            }
                        }
                    }
                    signal = signal_rx.recv() => {
                        let Some(signal) = signal else {
                            break;
                        };
                        let MediaSignal::PropertiesChanged(name) = signal;
                        // Property changes are per-player; refresh only the updated entry.
                        refresh_player_cache(&players, &mut cache, &name).await;
                        send_snapshot(&sender, &cache).await;
                        schedule_metadata_fallback(&cache, signal_tx.clone(), &name);
                    }
                    signal = owner_stream.next() => {
                        let Some(signal) = signal else {
                            break;
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
                                &connection,
                                &config,
                                &signal_tx,
                                &mut players,
                                &mut cache,
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
        });
    });

    Some(MediaHandle {
        command_tx: Some(command_tx),
    })
}

#[allow(clippy::too_many_arguments)]
async fn apply_owner_change(
    name: &str,
    new_owner: Option<&str>,
    connection: &Connection,
    config: &MediaConfig,
    signal_tx: &UnboundedSender<MediaSignal>,
    players: &mut HashMap<String, PlayerState>,
    cache: &mut HashMap<String, MediaInfo>,
    sender: &async_channel::Sender<UiEvent>,
) -> zbus::Result<()> {
    if !name.starts_with(MPRIS_PREFIX) {
        return Ok(());
    }

    if !is_allowed_player(name, config) {
        if players.remove(name).is_some() {
            cache.remove(name);
            send_snapshot(sender, cache).await;
        }
        return Ok(());
    }

    let has_owner = new_owner.map(|owner| !owner.is_empty()).unwrap_or(false);
    if !has_owner {
        if players.remove(name).is_some() {
            cache.remove(name);
            send_snapshot(sender, cache).await;
        }
        return Ok(());
    }

    if players.contains_key(name) {
        return Ok(());
    }

    if let Some(state) = build_player_state(connection, name).await? {
        spawn_properties_listener(
            state.properties.clone(),
            name.to_string(),
            signal_tx.clone(),
        );
        players.insert(name.to_string(), state);
        refresh_player_cache(players, cache, name).await;
        send_snapshot(sender, cache).await;
        schedule_metadata_fallback(cache, signal_tx.clone(), name);
    }

    Ok(())
}
