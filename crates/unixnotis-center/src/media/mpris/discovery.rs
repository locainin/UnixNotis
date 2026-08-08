//! Discovery and removal of admitted MPRIS players

use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;

use futures_util::stream::{self, StreamExt};
use tokio::sync::mpsc::Sender;
use tokio::time::Instant;
use tracing::warn;
use unixnotis_core::{MediaConfig, PanelDebugLevel};
use zbus::fdo::DBusProxy;
use zbus::Connection;

use super::constants::MAX_MPRIS_PLAYERS;
use super::fairness::MprisFairnessState;
use super::inventory::{
    admit_fairness_candidate, build_dbus_player_state, insert_player_state, FairnessAdmission,
    PlayerStateBuilder,
};
use super::player::{resolve_player_owner, OwnerProbe};
use super::selection::{is_discoverable_player, select_player_names};
use super::PlayerState;
use crate::diagnostics::panel_debug as debug;
use crate::media::runtime::MediaSignal;

pub(super) struct DiscoveryState<'a> {
    pub players: &'a mut HashMap<String, PlayerState>,
    pub discovery_cursor: &'a mut usize,
    pub fairness: &'a mut MprisFairnessState,
}

pub(in crate::media) async fn refresh_players(
    connection: &Connection,
    dbus_proxy: &DBusProxy<'_>,
    config: &MediaConfig,
    signal_tx: &Sender<MediaSignal>,
    players: &mut HashMap<String, PlayerState>,
    discovery_cursor: &mut usize,
    fairness: &mut MprisFairnessState,
) -> zbus::Result<()> {
    refresh_players_with_builder(
        connection,
        dbus_proxy,
        config,
        signal_tx,
        DiscoveryState {
            players,
            discovery_cursor,
            fairness,
        },
        build_dbus_player_state,
    )
    .await
}

pub(in crate::media) async fn refresh_players_with_builder(
    connection: &Connection,
    dbus_proxy: &DBusProxy<'_>,
    config: &MediaConfig,
    signal_tx: &Sender<MediaSignal>,
    state: DiscoveryState<'_>,
    build_player: PlayerStateBuilder,
) -> zbus::Result<()> {
    let DiscoveryState {
        players,
        discovery_cursor,
        fairness,
    } = state;
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
    let mut eligible = Vec::<(String, OwnerProbe)>::new();
    let mut candidate_owners = HashSet::new();
    for (name, owner) in probed {
        let Some(owner) = owner else {
            failed_probes = failed_probes.saturating_add(1);
            continue;
        };
        // Several aliases can resolve to one connection; retain one stable alias
        if owners.contains(&owner.unique_owner)
            || !candidate_owners.insert(owner.unique_owner.clone())
        {
            continue;
        }
        eligible.push((name, owner));
    }

    // Build ordinary admissions until successful states fill the owner capacity
    while owners.len() < MAX_MPRIS_PLAYERS && !eligible.is_empty() {
        let (name, owner) = eligible.remove(0);
        let state = match build_player(connection, &name, config, owner).await {
            Ok(state) => state,
            Err(err) => {
                failed_probes = failed_probes.saturating_add(1);
                debug::log(PanelDebugLevel::Verbose, || {
                    format!("failed to build media player state for {name}: {err}")
                });
                continue;
            }
        };
        owners.extend(state.unique_owner.iter().cloned());
        insert_player_state(players, signal_tx, name, state);
    }

    // Starting the lease after normal admission also covers over-capacity startup inventories
    let fairness_rotation_due = fairness.rotation_due(
        owners.len() >= MAX_MPRIS_PLAYERS,
        !eligible.is_empty(),
        Instant::now(),
        signal_tx,
    );
    if fairness_rotation_due && !eligible.is_empty() {
        match admit_fairness_candidate(
            connection,
            config,
            signal_tx,
            players,
            fairness,
            eligible.remove(0),
            build_player,
        )
        .await
        {
            FairnessAdmission::Admitted {
                victim_name,
                candidate_name,
            } => {
                // Successful admission starts the next bounded opportunity
                fairness.complete_rotation(Instant::now(), true, signal_tx);
                debug::log(PanelDebugLevel::Info, || {
                    format!("media player lease rotated: {victim_name} -> {candidate_name}")
                });
            }
            FairnessAdmission::BuildFailed {
                candidate_name,
                error,
            } => {
                failed_probes = failed_probes.saturating_add(1);
                // A failed candidate leaves every healthy incumbent untouched
                fairness.retry_failed_rotation(Instant::now(), signal_tx);
                debug::log(PanelDebugLevel::Verbose, || {
                    format!(
                        "failed to build fairness media player state for {candidate_name}: {error}"
                    )
                });
            }
            FairnessAdmission::NoVictim => {
                capacity_skipped = capacity_skipped.saturating_add(1);
                fairness.retry_failed_rotation(Instant::now(), signal_tx);
            }
        }
    }
    capacity_skipped = capacity_skipped.saturating_add(eligible.len());
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
