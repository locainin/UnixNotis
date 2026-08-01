//! Discovery and removal of admitted MPRIS players

use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;

use futures_util::stream::{self, StreamExt};
use tokio::sync::mpsc::Sender;
use tracing::warn;
use unixnotis_core::{MediaConfig, PanelDebugLevel};
use zbus::fdo::DBusProxy;
use zbus::Connection;

use super::constants::{MAX_MPRIS_CANDIDATES_PER_PASS, MAX_MPRIS_PLAYERS, MPRIS_PREFIX};
use super::player::{build_player_state_for_owner, resolve_player_owner, OwnerProbe};
use super::{is_allowed_player, spawn_properties_listener, PlayerState};
use crate::diagnostics::panel_debug as debug;
use crate::media::runtime::MediaSignal;

pub(in crate::media) async fn refresh_players(
    connection: &Connection,
    dbus_proxy: &DBusProxy<'_>,
    config: &MediaConfig,
    signal_tx: &Sender<MediaSignal>,
    players: &mut HashMap<String, PlayerState>,
    discovery_cursor: &mut usize,
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

    // Keep active names, then rotate through the remaining sorted names
    let tracked = players.keys().cloned().collect::<HashSet<_>>();
    let allowed = select_player_names(allowed, &tracked, discovery_cursor);
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
    // Owner-only probes are bounded before any full player construction
    let mut probed = stream::iter(names_to_probe)
        .map(|name| async move {
            let result = resolve_player_owner(connection, &name).await;
            (name, result)
        })
        .buffer_unordered(4)
        .collect::<Vec<_>>()
        .await;
    // Concurrency must not change which alias wins owner deduplication
    probed.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    let mut failed_probes = 0usize;
    let mut capacity_skipped = 0usize;
    let mut selected = Vec::<(String, OwnerProbe)>::new();
    for (name, owner) in probed {
        let Some(owner) = owner else {
            failed_probes = failed_probes.saturating_add(1);
            continue;
        };
        // Several aliases can resolve to one connection; retain one stable alias
        if !owners.insert(owner.unique_owner.clone()) {
            continue;
        }
        if owner_capacity_exceeded(owners.len(), MAX_MPRIS_PLAYERS) {
            owners.remove(&owner.unique_owner);
            capacity_skipped = capacity_skipped.saturating_add(1);
            continue;
        }
        selected.push((name, owner));
    }

    // Full construction runs once per selected owner, never once per alias
    for (name, owner) in selected {
        let state = match build_player_state_for_owner(connection, &name, config, owner).await {
            Ok(state) => state,
            Err(err) => {
                failed_probes = failed_probes.saturating_add(1);
                debug::log(PanelDebugLevel::Verbose, || {
                    format!("failed to build media player state for {name}: {err}")
                });
                continue;
            }
        };
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

pub(super) fn select_player_names(
    names: HashSet<String>,
    tracked: &HashSet<String>,
    cursor: &mut usize,
) -> Vec<String> {
    let mut names = names.into_iter().collect::<Vec<_>>();
    names.sort_unstable();

    let mut selected = names
        .iter()
        .filter(|name| tracked.contains(*name))
        .cloned()
        .collect::<Vec<_>>();
    let remaining = names
        .into_iter()
        .filter(|name| !tracked.contains(name))
        .collect::<Vec<_>>();
    let room = MAX_MPRIS_CANDIDATES_PER_PASS.saturating_sub(selected.len());
    if room == 0 || remaining.is_empty() {
        return selected;
    }

    let start = *cursor % remaining.len();
    let count = room.min(remaining.len());
    selected.extend((0..count).map(|offset| remaining[(start + offset) % remaining.len()].clone()));
    *cursor = (start + count) % remaining.len();
    selected
}

pub(super) const fn owner_capacity_exceeded(owner_count: usize, capacity: usize) -> bool {
    owner_count > capacity
}
