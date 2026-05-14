use std::collections::HashMap;

use tokio::sync::mpsc;
use tracing::warn;
use unixnotis_core::MediaConfig;
use zbus::fdo::DBusProxy;
use zbus::Connection;

use crate::dbus::UiEvent;

use super::bus::{
    build_player_state, handle_command, is_allowed_player, refresh_players,
    spawn_properties_listener, PlayerState,
};
use super::cache::{refresh_cache, refresh_player_cache, MediaCacheMergeMode};
use super::event_loop::MediaRuntimeState;
use super::schedule::{
    cancel_delayed_refresh, prune_delayed_refreshes, schedule_command_refresh,
    schedule_metadata_fallback, schedule_metadata_fallbacks, DelayedRefreshTasks,
};
use super::snapshot::send_snapshot_if_changed;
use super::{MediaCommand, MediaRefreshOrigin, MediaSignal, MPRIS_PREFIX};

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
) -> zbus::Result<()> {
    // Owner changes are the one place where the loop has to answer
    // "did a player appear or disappear" instead of "did one player update"
    if !name.starts_with(MPRIS_PREFIX) {
        // Ignore unrelated bus names so the loop only tracks real MPRIS owners
        return Ok(());
    }

    if !is_allowed_player(name, config) {
        // A now-disallowed player must disappear from the UI right away
        remove_player(name, state, sender).await;
        return Ok(());
    }

    let has_owner = new_owner.map(|owner| !owner.is_empty()).unwrap_or(false);
    if !has_owner {
        // Losing the bus owner means the player has gone away
        remove_player(name, state, sender).await;
        return Ok(());
    }

    if state.players.contains_key(name) {
        // Existing entries keep their listener and cache until the next real update
        // This avoids tearing down and rebuilding watchers on harmless owner churn
        return Ok(());
    }

    if let Some(player_state) = build_player_state(connection, name, config).await? {
        // The listener is started before the state is published so late property
        // traffic does not slip in between player creation and cache refresh
        spawn_properties_listener(
            player_state.properties.clone(),
            name.to_string(),
            signal_tx.clone(),
            player_state.listener_cancel.subscribe(),
        );
        state.players.insert(name.to_string(), player_state);
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

    Ok(())
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

fn should_publish_immediate_command_snapshot(command: &MediaCommand) -> bool {
    // Track skip commands often produce one partial metadata frame before the real update settles
    // Let the bus event or bounded retry publish those instead of flashing a blank card
    matches!(command, MediaCommand::PlayPause { .. })
}

fn merge_mode_for_signal(origin: MediaRefreshOrigin) -> MediaCacheMergeMode {
    match origin {
        // Native property bursts can still be mid-transition
        MediaRefreshOrigin::Bus => MediaCacheMergeMode::Transitioning,
        // Delayed retries are where sparse snapshots get reconciled to their final state
        MediaRefreshOrigin::Fallback => MediaCacheMergeMode::Stable,
    }
}

#[cfg(test)]
#[path = "tests/loop_events.rs"]
mod tests;
