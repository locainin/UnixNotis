//! Discovery and removal of admitted MPRIS players

use std::collections::{HashMap, HashSet};

use tokio::sync::mpsc::Sender;
use tracing::warn;
use unixnotis_core::{MediaConfig, PanelDebugLevel};
use zbus::fdo::DBusProxy;
use zbus::Connection;

use super::constants::MPRIS_PREFIX;
use super::{build_player_state, is_allowed_player, spawn_properties_listener, PlayerState};
use crate::diagnostics::panel_debug as debug;
use crate::media::runtime::MediaSignal;

pub(in crate::media) async fn refresh_players(
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
        // Apply allow, deny, and browser policy before creating proxies or listener tasks
        if !is_allowed_player(&name, config) {
            continue;
        }
        allowed.insert(name);
    }

    // Remove players that no longer exist on the bus to avoid stale UI cards
    let mut removed_names = Vec::new();
    for name in players.keys() {
        if !allowed.contains(name) {
            removed_names.push(name.clone());
        }
    }
    for name in &removed_names {
        if let Some(state) = players.remove(name) {
            // Signal the background listener to shut down promptly
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
            // Each player gets a properties listener so updates stay event-driven
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
