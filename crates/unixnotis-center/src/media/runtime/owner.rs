//! MPRIS owner replacement and player removal

use tokio::sync::mpsc;
use unixnotis_core::MediaConfig;
use zbus::Connection;

use super::cache::{refresh_player_cache, MediaCacheMergeMode};
use super::schedule::{cancel_delayed_refresh, schedule_metadata_fallback};
use super::snapshot::send_snapshot_if_changed;
use super::state::MediaRuntimeState;
use super::MediaSignal;
use crate::control::UiEvent;
use crate::media::mpris::{
    build_player_state, is_allowed_player, spawn_properties_listener, MAX_MPRIS_PLAYERS,
    MPRIS_PREFIX,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OwnerChangeOutcome {
    // The announced owner is represented by the current player state
    Applied,
    // The announcement intentionally leaves no player under this bus name
    Removed,
    // Owner identity changed during probing and needs one bounded discovery retry
    RetryNeeded,
}

pub(super) async fn apply_owner_change(
    name: &str,
    new_owner: Option<&str>,
    connection: &Connection,
    config: &MediaConfig,
    signal_tx: &mpsc::Sender<MediaSignal>,
    state: &mut MediaRuntimeState,
    sender: &async_channel::Sender<UiEvent>,
) -> zbus::Result<OwnerChangeOutcome> {
    // Owner changes answer whether a player appeared, disappeared, or was replaced
    if !name.starts_with(MPRIS_PREFIX) {
        return Ok(OwnerChangeOutcome::Applied);
    }

    if !is_allowed_player(name, config) {
        // A now-disallowed player must disappear from the UI right away
        remove_player(name, state, sender).await;
        return Ok(OwnerChangeOutcome::Removed);
    }

    let has_owner = new_owner.is_some_and(|owner| !owner.is_empty());
    if !has_owner {
        // Losing the bus owner means the player has gone away
        remove_player(name, state, sender).await;
        return Ok(OwnerChangeOutcome::Removed);
    }

    if should_retry_for_capacity(state.players.contains_key(name), state.players.len()) {
        // Full discovery will choose the deterministic prefix on the next pass
        return Ok(OwnerChangeOutcome::RetryNeeded);
    }

    if state
        .players
        .get(name)
        .is_some_and(|player| owner_is_unchanged(player.unique_owner.as_deref(), new_owner))
    {
        // Duplicate owner announcements do not need to rebuild a healthy listener
        return Ok(OwnerChangeOutcome::Applied);
    }

    let removed_previous = if let Some(previous) = state.players.remove(name) {
        // A replacement owner invalidates every process-bound proxy and policy decision
        let _ = previous.listener_cancel.send(true);
        cancel_delayed_refresh(&mut state.delayed_refreshes, name);
        state.cache.remove(name);
        true
    } else {
        false
    };

    let rebuilt = build_player_state(connection, name, config).await;
    if let Ok(Some(player_state)) = rebuilt.as_ref() {
        let duplicate_owner = state.players.iter().any(|(existing_name, existing)| {
            owner_is_duplicate(
                existing_name,
                name,
                existing.unique_owner.as_deref(),
                player_state.unique_owner.as_deref(),
            )
        });
        if duplicate_owner {
            if removed_previous {
                // The old alias was removed before deduplication and still needs a UI update
                send_snapshot_if_changed(sender, &state.cache, &mut state.last_snapshot).await;
            }
            return Ok(OwnerChangeOutcome::Applied);
        }
        // Start the listener before publishing state so late property traffic is retained
        spawn_properties_listener(
            player_state.properties.clone(),
            name.to_string(),
            signal_tx.clone(),
            player_state.listener_cancel.subscribe(),
        );
        state.players.insert(name.to_string(), player_state.clone());
        refresh_player_cache(
            &state.players,
            &mut state.cache,
            name,
            MediaCacheMergeMode::Stable,
        )
        .await;
        send_snapshot_if_changed(sender, &state.cache, &mut state.last_snapshot).await;
        schedule_metadata_fallback(
            &mut state.delayed_refreshes,
            &state.cache,
            signal_tx.clone(),
            name,
        );
    }

    // Removing a prior cache must reach GTK even when replacement probing fails
    let outcome = match rebuilt {
        Ok(state) => owner_rebuild_outcome(state.is_some()),
        Err(err) => {
            if removed_previous {
                send_snapshot_if_changed(sender, &state.cache, &mut state.last_snapshot).await;
            }
            return Err(err);
        }
    };
    if replacement_removal_needs_snapshot(removed_previous, outcome) {
        send_snapshot_if_changed(sender, &state.cache, &mut state.last_snapshot).await;
    }
    Ok(outcome)
}

pub(super) const fn owner_rebuild_outcome(rebuilt: bool) -> OwnerChangeOutcome {
    if rebuilt {
        OwnerChangeOutcome::Applied
    } else {
        OwnerChangeOutcome::RetryNeeded
    }
}

pub(super) const fn replacement_removal_needs_snapshot(
    removed_previous: bool,
    outcome: OwnerChangeOutcome,
) -> bool {
    removed_previous && !matches!(outcome, OwnerChangeOutcome::Applied)
}

pub(super) fn owner_is_unchanged(
    current_owner: Option<&str>,
    announced_owner: Option<&str>,
) -> bool {
    current_owner.is_some() && current_owner == announced_owner
}

pub(super) const fn should_retry_for_capacity(tracked: bool, player_count: usize) -> bool {
    !tracked && player_count >= MAX_MPRIS_PLAYERS
}

pub(super) fn owner_is_duplicate(
    existing_name: &str,
    requested_name: &str,
    existing_owner: Option<&str>,
    requested_owner: Option<&str>,
) -> bool {
    existing_name != requested_name && existing_owner == requested_owner
}

async fn remove_player(
    name: &str,
    state: &mut MediaRuntimeState,
    sender: &async_channel::Sender<UiEvent>,
) {
    let Some(player) = state.players.remove(name) else {
        return;
    };
    // The listener must stop as soon as the player stops being tracked
    let _ = player.listener_cancel.send(true);
    cancel_delayed_refresh(&mut state.delayed_refreshes, name);
    state.cache.remove(name);
    send_snapshot_if_changed(sender, &state.cache, &mut state.last_snapshot).await;
}
