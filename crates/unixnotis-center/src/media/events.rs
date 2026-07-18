use std::collections::HashMap;

use tokio::sync::mpsc;
use tracing::warn;
use unixnotis_core::MediaConfig;
use zbus::fdo::DBusProxy;
use zbus::Connection;

use crate::control::UiEvent;

use super::bus::{
    build_player_state, handle_command, is_allowed_player, refresh_players,
    spawn_properties_listener, PlayerState,
};
use super::runtime::cache::{refresh_cache, refresh_player_cache, MediaCacheMergeMode};
use super::runtime::r#loop::MediaRuntimeState;
use super::runtime::schedule::{
    cancel_delayed_refresh, prune_delayed_refreshes, schedule_command_refresh,
    schedule_metadata_fallback, schedule_metadata_fallbacks, DelayedRefreshTasks,
};
use super::runtime::snapshot::send_snapshot_if_changed;
use super::runtime::{MediaRefreshOrigin, MediaSignal};
use super::{identifiers::MPRIS_PREFIX, MediaCommand};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OwnerChangeOutcome {
    // The announced owner is represented by the current player state
    Applied,
    // The announcement intentionally leaves no player under this bus name
    Removed,
    // Owner identity changed during probing and needs one bounded discovery retry
    RetryNeeded,
}

pub(super) async fn refresh_all_players(
    connection: &Connection,
    dbus_proxy: &DBusProxy<'_>,
    config: &MediaConfig,
    signal_tx: &mpsc::Sender<MediaSignal>,
    state: &mut MediaRuntimeState,
    sender: &async_channel::Sender<UiEvent>,
) {
    // Full refresh owns the "what players exist right now" question
    // Everything else in this file works from that settled player map
    if let Err(err) = refresh_players(
        connection,
        dbus_proxy,
        config,
        signal_tx,
        &mut state.players,
    )
    .await
    {
        warn!(?err, "failed to refresh media players");
    }
    // Remove retries for players that disappeared during refresh
    prune_player_refreshes(&mut state.delayed_refreshes, &state.players);
    // Cache rebuild happens after player discovery so the snapshot stays aligned
    refresh_cache(&state.players, &mut state.cache).await;
    send_snapshot_if_changed(sender, &state.cache, &mut state.last_snapshot).await;
    schedule_metadata_fallbacks(
        &mut state.delayed_refreshes,
        &state.cache,
        signal_tx.clone(),
    );
}

pub(super) async fn handle_runtime_command(
    state: &mut MediaRuntimeState,
    signal_tx: &mpsc::Sender<MediaSignal>,
    sender: &async_channel::Sender<UiEvent>,
    command: MediaCommand,
) {
    // Command handling is intentionally split from signal handling because
    // button-triggered refresh rules are stricter than bus-driven bursts
    let publish_immediately = should_publish_immediate_command_snapshot(&command);
    if let Ok(Some(name)) = handle_command(&state.players, command).await {
        if publish_immediately {
            // Play and pause changes are simple enough to reflect without waiting for retries
            refresh_player_cache(
                &state.players,
                &mut state.cache,
                &name,
                MediaCacheMergeMode::Transitioning,
            )
            .await;
            send_snapshot_if_changed(sender, &state.cache, &mut state.last_snapshot).await;
        }
        schedule_command_refresh(
            &mut state.delayed_refreshes,
            &state.cache,
            signal_tx.clone(),
            &name,
        );
    }
}

pub(super) async fn handle_runtime_signal(
    state: &mut MediaRuntimeState,
    signal_tx: &mpsc::Sender<MediaSignal>,
    sender: &async_channel::Sender<UiEvent>,
    signal: MediaSignal,
) {
    // Signal payloads already name the one player that changed, so the loop
    // can stay cheap and avoid rebuilding the whole cache on every property burst
    let MediaSignal::PropertiesChanged { bus_name, origin } = signal;
    // Property changes refresh one player only, which keeps updates cheap
    refresh_player_cache(
        &state.players,
        &mut state.cache,
        &bus_name,
        merge_mode_for_signal(origin),
    )
    .await;
    send_snapshot_if_changed(sender, &state.cache, &mut state.last_snapshot).await;
    if should_schedule_metadata_fallback(origin) {
        // Bus-driven changes can need one bounded late-art sweep
        schedule_metadata_fallback(
            &mut state.delayed_refreshes,
            &state.cache,
            signal_tx.clone(),
            &bus_name,
        );
    }
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
    // Owner changes are the one place where the loop has to answer
    // "did a player appear or disappear" instead of "did one player update"
    if !name.starts_with(MPRIS_PREFIX) {
        // Ignore unrelated bus names so the loop only tracks real MPRIS owners
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
        // The listener is started before the state is published so late property
        // traffic does not slip in between player creation and cache refresh
        spawn_properties_listener(
            player_state.properties.clone(),
            name.to_string(),
            signal_tx.clone(),
            player_state.listener_cancel.subscribe(),
        );
        state.players.insert(name.to_string(), player_state.clone());
        // A late-joining player still needs one snapshot pass through the cache
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

    // Removing the prior cache must reach GTK even when replacement probing fails
    let outcome = match rebuilt {
        Ok(state) => owner_rebuild_outcome(state.is_some()),
        Err(err) => {
            if removed_previous {
                // The stale card must disappear even when proxy construction itself errors
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

const fn owner_rebuild_outcome(rebuilt: bool) -> OwnerChangeOutcome {
    if rebuilt {
        OwnerChangeOutcome::Applied
    } else {
        OwnerChangeOutcome::RetryNeeded
    }
}

const fn replacement_removal_needs_snapshot(
    removed_previous: bool,
    outcome: OwnerChangeOutcome,
) -> bool {
    removed_previous && !matches!(outcome, OwnerChangeOutcome::Applied)
}

fn owner_is_unchanged(current_owner: Option<&str>, announced_owner: Option<&str>) -> bool {
    current_owner.is_some() && current_owner == announced_owner
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
    // Retry work for the removed player is no longer useful
    cancel_delayed_refresh(&mut state.delayed_refreshes, name);
    state.cache.remove(name);
    send_snapshot_if_changed(sender, &state.cache, &mut state.last_snapshot).await;
}

fn prune_player_refreshes(
    delayed_refreshes: &mut DelayedRefreshTasks,
    players: &HashMap<String, PlayerState>,
) {
    // First drop completed tasks so the map tracks only live retry plans
    prune_delayed_refreshes(delayed_refreshes);
    // Missing players should not keep sleeping retry tasks around
    delayed_refreshes.retain(|name, task| {
        if players.contains_key(name) {
            return true;
        }
        // Abort before dropping the entry so late wakeups do not outlive the player map
        task.abort();
        false
    });
}

fn should_schedule_metadata_fallback(origin: MediaRefreshOrigin) -> bool {
    // Synthetic retries already represent the bounded fallback plan
    // Re-arming here would collapse into a permanent 250 ms self-refresh loop
    origin == MediaRefreshOrigin::Bus
}

const fn should_publish_immediate_command_snapshot(command: &MediaCommand) -> bool {
    // Track skip commands often produce one partial metadata frame before the real update settles
    // Let the bus event or bounded retry publish those instead of flashing a blank card
    matches!(command, MediaCommand::PlayPause { .. })
}

const fn merge_mode_for_signal(origin: MediaRefreshOrigin) -> MediaCacheMergeMode {
    match origin {
        // Native property bursts can still be mid-transition
        MediaRefreshOrigin::Bus => MediaCacheMergeMode::Transitioning,
        // Delayed retries are where sparse snapshots get reconciled to their final state
        MediaRefreshOrigin::Fallback => MediaCacheMergeMode::Stable,
    }
}

#[cfg(test)]
#[path = "tests/events.rs"]
mod tests;
