use std::collections::{HashMap, HashSet};

use futures_util::StreamExt;
use tokio::sync::{mpsc::Sender, watch};
use tracing::warn;
use unixnotis_core::{MediaConfig, PanelDebugLevel};
use zbus::fdo::{DBusProxy, PropertiesProxy};
use zbus::{Connection, Proxy, ProxyBuilder};

use super::policy::{detect_browser_family, remote_art_allowed};
use super::{
    MediaCommand, MediaRefreshOrigin, MediaSignal, MPRIS_APP, MPRIS_PATH, MPRIS_PLAYER,
    MPRIS_PREFIX,
};
use crate::debug;

#[derive(Clone)]
pub(super) struct PlayerState {
    pub(super) bus_name: String,
    pub(super) identity: String,
    pub(super) browser_family: Option<String>,
    pub(super) owner_pid: Option<u32>,
    pub(super) remote_art_allowed: bool,
    pub(super) player: Proxy<'static>,
    pub(super) properties: PropertiesProxy<'static>,
    // Cancellation sender for the properties listener task.
    pub(super) listener_cancel: watch::Sender<bool>,
}

pub(super) async fn refresh_players(
    connection: &Connection,
    dbus_proxy: &DBusProxy<'_>,
    config: &MediaConfig,
    signal_tx: &Sender<MediaSignal>,
    players: &mut HashMap<String, PlayerState>,
) -> zbus::Result<()> {
    let names = dbus_proxy.list_names().await?;
    let mut allowed = HashSet::new();
    for name in names {
        let name = name.to_string();
        if !name.starts_with(MPRIS_PREFIX) {
            continue;
        }
        // Apply allow/deny/browser policy before creating proxies or listener tasks
        if !is_allowed_player(&name, config) {
            continue;
        }
        allowed.insert(name);
    }

    // Remove players that no longer exist on the bus to avoid stale UI cards.
    let mut removed_names: Vec<String> = Vec::new();
    for name in players.keys() {
        if !allowed.contains(name) {
            removed_names.push(name.clone());
        }
    }
    for name in &removed_names {
        if let Some(state) = players.remove(name) {
            // Signal the background listener to shut down promptly.
            let _ = state.listener_cancel.send(true);
        }
    }
    if !removed_names.is_empty() {
        debug::log(PanelDebugLevel::Info, || {
            format!("media players removed: {}", removed_names.len())
        });
    }

    for name in allowed {
        if players.contains_key(&name) {
            continue;
        }
        // New players are probed once before entering the live cache
        let state = match build_player_state(connection, &name, config).await {
            Ok(state) => state,
            Err(err) => {
                warn!(?err, player = %name, "failed to build media player state");
                continue;
            }
        };
        if let Some(state) = state {
            // Each player gets a properties listener so updates stay event-driven.
            spawn_properties_listener(
                state.properties.clone(),
                name.clone(),
                signal_tx.clone(),
                state.listener_cancel.subscribe(),
            );
            players.insert(name.clone(), state);
            debug::log(PanelDebugLevel::Info, || {
                format!("media player added: {name}")
            });
        }
    }

    Ok(())
}

pub(super) fn spawn_properties_listener(
    properties: PropertiesProxy<'static>,
    bus_name: String,
    signal_tx: Sender<MediaSignal>,
    mut cancel_rx: watch::Receiver<bool>,
) {
    tokio::spawn(async move {
        let mut stream = match properties.receive_properties_changed().await {
            Ok(stream) => stream,
            Err(err) => {
                warn!(?err, "failed to subscribe to media properties");
                return;
            }
        };
        loop {
            tokio::select! {
                result = cancel_rx.changed() => {
                    // Exit promptly when the player is removed or cancellation is requested.
                    if result.is_err() || *cancel_rx.borrow() {
                        break;
                    }
                }
                update = stream.next() => {
                    let Some(update) = update else {
                        break;
                    };
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
                    if signal_tx
                        .send(MediaSignal::PropertiesChanged {
                            bus_name: bus_name.clone(),
                            origin: MediaRefreshOrigin::Bus,
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
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

    // Ignore unrelated property churn so browser players do not wake the panel constantly
    if changed.keys().any(|key| KEYS.contains(key)) {
        return true;
    }
    invalidated.iter().any(|key| KEYS.contains(key))
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
    config: &MediaConfig,
) -> zbus::Result<Option<PlayerState>> {
    let identity = fetch_identity(connection, name)
        .await
        .unwrap_or_else(|| name.to_string());
    // DBus owner data is captured once so snapshots do not need another bus round trip
    // Browser bridges may later override this PID with a stronger metadata source PID
    let (owner_pid, owner_executable) = resolve_player_owner(connection, name).await;
    let browser_family = detect_browser_family(&identity, name, &config.browser_tokens);
    let remote_art_allowed = remote_art_allowed(
        browser_family.as_deref(),
        owner_executable.as_deref(),
        config.remote_art_policy,
    );
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
    let (listener_cancel, _listener_rx) = watch::channel(false);

    Ok(Some(PlayerState {
        bus_name: name.to_string(),
        identity,
        browser_family,
        owner_pid,
        remote_art_allowed,
        player,
        properties,
        listener_cancel,
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

async fn resolve_player_owner(
    connection: &Connection,
    name: &str,
) -> (Option<u32>, Option<String>) {
    // Some synthetic names cannot be converted into a DBus bus name
    // Treat those as unknown instead of rejecting the whole media player
    let Ok(bus_name) = zbus::names::BusName::try_from(name) else {
        return (None, None);
    };
    let Ok(proxy) = DBusProxy::new(connection).await else {
        return (None, None);
    };
    // The bus owner PID is useful for normal players and art trust policy
    // It is weaker than bridge metadata when a helper owns the MPRIS name
    let pid = proxy.get_connection_unix_process_id(bus_name).await.ok();
    let executable = match pid {
        Some(pid) => read_process_executable_path(pid)
            .await
            .map(|path| path.display().to_string()),
        None => None,
    };
    (pid, executable)
}

#[cfg(target_os = "linux")]
async fn read_process_executable_path(pid: u32) -> Option<std::path::PathBuf> {
    // Reading /proc keeps the trust hint tied to the real bus owner process
    tokio::fs::read_link(format!("/proc/{pid}/exe")).await.ok()
}

#[cfg(not(target_os = "linux"))]
async fn read_process_executable_path(_pid: u32) -> Option<std::path::PathBuf> {
    // Non-Linux builds degrade to local-file-only artwork
    None
}

pub(super) fn is_allowed_player(name: &str, config: &MediaConfig) -> bool {
    let lower = name.to_lowercase();
    if config.denylist.iter().any(|entry| lower.contains(entry)) {
        return false;
    }

    if !config.allowlist.is_empty() {
        return config.allowlist.iter().any(|entry| lower.contains(entry));
    }

    if !config.include_browsers && is_browser_name(&lower, &config.browser_tokens) {
        return false;
    }

    true
}

fn is_browser_name(lower: &str, browser_tokens: &[String]) -> bool {
    // Browser tokens match whole segments so short defaults do not overfire
    browser_tokens
        .iter()
        .any(|token| super::policy::token_matches_segment(lower, token))
}

#[cfg(test)]
#[path = "tests/bus.rs"]
mod tests;
