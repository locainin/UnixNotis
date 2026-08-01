//! Discovery and removal of admitted MPRIS players

use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;

use futures_util::stream::{self, StreamExt};
use tokio::sync::mpsc::Sender;
use tracing::warn;
use unixnotis_core::{MediaConfig, PanelDebugLevel};
use zbus::fdo::DBusProxy;
use zbus::Connection;

use super::constants::{MAX_MPRIS_PLAYERS, MPRIS_PREFIX};
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
        // Apply allow, deny, and browser policy before creating proxies or listener tasks
        if !is_discoverable_player(&name, config) {
            continue;
        }
        allowed.insert(name);
    }

    // Owner capacity is enforced after probing so aliases cannot occupy a
    // deterministic name prefix and starve an unrelated player
    let allowed = select_player_names(allowed);
    let allowed_set = allowed.iter().map(String::as_str).collect::<HashSet<_>>();

    // Remove players that no longer exist on the bus to avoid stale UI cards
    let mut removed_names = Vec::new();
    for name in players.keys() {
        if !allowed_set.contains(name.as_str()) {
            removed_names.push(name.clone());
        }
    }
    for name in &removed_names {
        if let Some(state) = players.remove(name) {
            // Signal the background listener to shut down promptly
            let _ = state.listener_cancel.send(true);
        }
    }
    if let Some(removed_count) = NonZeroUsize::new(removed_names.len()) {
        debug::log(PanelDebugLevel::Info, || {
            format!("media players removed: {removed_count}")
        });
    }

    let mut owners = players
        .values()
        .filter_map(|player| player.unique_owner.clone())
        .collect::<HashSet<_>>();
    let names_to_probe = allowed
        .iter()
        .filter(|name| !players.contains_key(*name))
        .cloned()
        .collect::<Vec<_>>();
    let mut probed = stream::iter(names_to_probe)
        .map(|name| async move {
            let result = build_player_state(connection, &name, config).await;
            (name, result)
        })
        .buffer_unordered(4)
        .collect::<Vec<_>>()
        .await;
    // Concurrency must not change which alias wins owner deduplication
    probed.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    let mut failed_probes = 0usize;
    let mut capacity_skipped = 0usize;
    for (name, result) in probed {
        // New players are probed concurrently, but admitted state is committed in name order
        let state = match result {
            Ok(state) => state,
            Err(err) => {
                failed_probes = failed_probes.saturating_add(1);
                debug::log(PanelDebugLevel::Verbose, || {
                    format!("failed to build media player state for {name}: {err}")
                });
                continue;
            }
        };
        if let Some(state) = state {
            let owner_is_tracked = state
                .unique_owner
                .as_ref()
                .is_some_and(|owner| owners.contains(owner));
            if should_skip_for_owner_capacity(owners.len(), MAX_MPRIS_PLAYERS, owner_is_tracked) {
                // The owner was resolved, but the bounded state set is full
                capacity_skipped = capacity_skipped.saturating_add(1);
                continue;
            }
            if state
                .unique_owner
                .as_ref()
                .is_some_and(|owner| !owners.insert(owner.clone()))
            {
                // Several well-known names may point to one owner; one listener is enough
                continue;
            }
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
    if failed_probes > 0 {
        warn!(
            failed = failed_probes,
            "one or more MPRIS player probes failed"
        );
    }
    if capacity_skipped > 0 {
        warn!(
            skipped = capacity_skipped,
            limit = MAX_MPRIS_PLAYERS,
            "MPRIS player capacity reached; additional owners were ignored"
        );
    }

    Ok(())
}

pub(super) fn is_discoverable_player(name: &str, config: &MediaConfig) -> bool {
    name.starts_with(MPRIS_PREFIX) && is_allowed_player(name, config)
}

pub(super) fn select_player_names(names: HashSet<String>) -> Vec<String> {
    let mut names: Vec<String> = names.into_iter().collect();
    names.sort_unstable();
    names
}

pub(super) const fn should_skip_for_owner_capacity(
    owner_count: usize,
    capacity: usize,
    owner_is_tracked: bool,
) -> bool {
    owner_count >= capacity && !owner_is_tracked
}
