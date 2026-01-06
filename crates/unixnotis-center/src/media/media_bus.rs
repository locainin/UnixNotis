//! D-Bus discovery and command handling for MPRIS players.
//!
//! Keeps bus interactions isolated from cache and UI update logic.

use std::collections::{HashMap, HashSet};

use futures_util::StreamExt;
use tokio::sync::mpsc::UnboundedSender;
use tracing::warn;
use unixnotis_core::{MediaConfig, PanelDebugLevel};
use zbus::fdo::{DBusProxy, PropertiesProxy};
use zbus::{Connection, Proxy, ProxyBuilder};

use super::{MediaCommand, MediaSignal, MPRIS_APP, MPRIS_PATH, MPRIS_PLAYER, MPRIS_PREFIX};
use crate::debug;

#[derive(Clone)]
pub(super) struct PlayerState {
    pub(super) bus_name: String,
    pub(super) identity: String,
    pub(super) player: Proxy<'static>,
    pub(super) properties: PropertiesProxy<'static>,
}

pub(super) async fn refresh_players(
    connection: &Connection,
    dbus_proxy: &DBusProxy<'_>,
    config: &MediaConfig,
    signal_tx: &UnboundedSender<MediaSignal>,
    players: &mut HashMap<String, PlayerState>,
) -> zbus::Result<()> {
    let names = dbus_proxy.list_names().await?;
    let mut allowed = HashSet::new();
    for name in names {
        let name = name.to_string();
        if !name.starts_with(MPRIS_PREFIX) {
            continue;
        }
        if !is_allowed_player(&name, config) {
            continue;
        }
        allowed.insert(name);
    }

    // Remove players that no longer exist on the bus to avoid stale UI cards.
    let before = players.len();
    players.retain(|name, _| allowed.contains(name));
    let removed = before.saturating_sub(players.len());
    if removed > 0 {
        debug::log(PanelDebugLevel::Info, || format!("media players removed: {removed}"));
    }

    for name in allowed {
        if players.contains_key(&name) {
            continue;
        }
        let state = match build_player_state(connection, &name).await {
            Ok(state) => state,
            Err(err) => {
                warn!(?err, player = %name, "failed to build media player state");
                continue;
            }
        };
        if let Some(state) = state {
            // Each player gets a properties listener so updates stay event-driven.
            spawn_properties_listener(state.properties.clone(), name.clone(), signal_tx.clone());
            players.insert(name.clone(), state);
            debug::log(PanelDebugLevel::Info, || format!("media player added: {name}"));
        }
    }

    Ok(())
}

pub(super) fn spawn_properties_listener(
    properties: PropertiesProxy<'static>,
    bus_name: String,
    signal_tx: UnboundedSender<MediaSignal>,
) {
    tokio::spawn(async move {
        let mut stream = match properties.receive_properties_changed().await {
            Ok(stream) => stream,
            Err(err) => {
                warn!(?err, "failed to subscribe to media properties");
                return;
            }
        };
        while let Some(update) = stream.next().await {
            let Ok(args) = update.args() else {
                continue;
            };
            if args.interface_name != MPRIS_PLAYER {
                continue;
            }
            if !is_relevant_media_change(&args.changed_properties, &args.invalidated_properties) {
                continue;
            }
            debug::log(PanelDebugLevel::Verbose, || {
                format!("media properties changed: {bus_name}")
            });
            let _ = signal_tx.send(MediaSignal::PropertiesChanged(bus_name.clone()));
        }
    });
}

fn is_relevant_media_change(
    changed: &HashMap<&str, zbus::zvariant::Value<'_>>,
    invalidated: &[&str],
) -> bool {
    const KEYS: [&str; 8] = [
        "Metadata",
        "PlaybackStatus",
        "LoopStatus",
        "Shuffle",
        "CanPlay",
        "CanPause",
        "CanGoNext",
        "CanGoPrevious",
    ];

    if changed.keys().any(|key| KEYS.contains(key)) {
        return true;
    }
    invalidated
        .iter()
        .any(|key| KEYS.contains(key))
}

pub(super) async fn handle_command(
    players: &HashMap<String, PlayerState>,
    command: MediaCommand,
) -> zbus::Result<Option<String>> {
    match command {
        MediaCommand::Refresh => Ok(None),
        MediaCommand::PlayPause { bus_name } => {
            if let Some(state) = players.get(&bus_name) {
                debug::log(PanelDebugLevel::Info, || {
                    format!("media command: play/pause {bus_name}")
                });
                // The returned bus name triggers a fast refresh for the targeted player.
                let _value: () = state.player.call("PlayPause", &()).await?;
                return Ok(Some(bus_name));
            }
            Ok(None)
        }
        MediaCommand::Next { bus_name } => {
            if let Some(state) = players.get(&bus_name) {
                debug::log(PanelDebugLevel::Info, || {
                    format!("media command: next {bus_name}")
                });
                // The returned bus name triggers a fast refresh for the targeted player.
                let _value: () = state.player.call("Next", &()).await?;
                return Ok(Some(bus_name));
            }
            Ok(None)
        }
        MediaCommand::Previous { bus_name } => {
            if let Some(state) = players.get(&bus_name) {
                debug::log(PanelDebugLevel::Info, || {
                    format!("media command: previous {bus_name}")
                });
                // The returned bus name triggers a fast refresh for the targeted player.
                let _value: () = state.player.call("Previous", &()).await?;
                return Ok(Some(bus_name));
            }
            Ok(None)
        }
    }
}

pub(super) async fn build_player_state(
    connection: &Connection,
    name: &str,
) -> zbus::Result<Option<PlayerState>> {
    let identity = fetch_identity(connection, name)
        .await
        .unwrap_or_else(|| name.to_string());
    let player = ProxyBuilder::new(connection)
        .destination(name.to_string())?
        .path(MPRIS_PATH)?
        .interface(MPRIS_PLAYER)?
        .build()
        .await?;
    let properties = PropertiesProxy::builder(connection)
        .destination(name.to_string())?
        .path(MPRIS_PATH)?
        .build()
        .await?;

    Ok(Some(PlayerState {
        bus_name: name.to_string(),
        identity,
        player,
        properties,
    }))
}

async fn fetch_identity(connection: &Connection, name: &str) -> Option<String> {
    let proxy: Proxy<'static> = ProxyBuilder::new(connection)
        .destination(name.to_string())
        .ok()?
        .path(MPRIS_PATH)
        .ok()?
        .interface(MPRIS_APP)
        .ok()?
        .build()
        .await
        .ok()?;
    proxy.get_property("Identity").await.ok()
}

pub(super) fn is_allowed_player(name: &str, config: &MediaConfig) -> bool {
    let lower = name.to_lowercase();
    if config
        .denylist
        .iter()
        .any(|entry| lower.contains(&entry.to_lowercase()))
    {
        return false;
    }

    if !config.allowlist.is_empty() {
        return config
            .allowlist
            .iter()
            .any(|entry| lower.contains(&entry.to_lowercase()));
    }

    if !config.include_browsers {
        let browser_tokens = ["firefox", "brave", "chromium", "chrome", "vivaldi"];
        if browser_tokens.iter().any(|token| lower.contains(token)) {
            return false;
        }
    }

    true
}
