//! Runtime command and property-signal dispatch

use tokio::sync::mpsc;

use super::cache::{refresh_player_cache, MediaCacheMergeMode};
use super::schedule::{schedule_command_refresh, schedule_metadata_fallback};
use super::snapshot::send_snapshot_if_changed;
use super::state::MediaRuntimeState;
use super::{MediaRefreshOrigin, MediaSignal};
use crate::control::UiEvent;
use crate::media::mpris::handle_command;
use crate::media::MediaCommand;

pub(super) async fn handle_runtime_command(
    state: &mut MediaRuntimeState,
    signal_tx: &mpsc::Sender<MediaSignal>,
    sender: &async_channel::Sender<UiEvent>,
    command: MediaCommand,
) {
    // Button-triggered refresh rules are stricter than bus-driven bursts
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
    bus_name: String,
    origin: MediaRefreshOrigin,
) {
    // Signal payloads name the one player that changed, avoiding a full cache rebuild
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

pub(super) const fn should_schedule_metadata_fallback(origin: MediaRefreshOrigin) -> bool {
    // Synthetic retries already represent the bounded fallback plan
    // Re-arming here would collapse into a permanent self-refresh loop
    matches!(origin, MediaRefreshOrigin::Bus)
}

pub(super) const fn should_publish_immediate_command_snapshot(command: &MediaCommand) -> bool {
    // Track skips often produce one partial metadata frame before the real update settles
    matches!(command, MediaCommand::PlayPause { .. })
}

pub(super) const fn merge_mode_for_signal(origin: MediaRefreshOrigin) -> MediaCacheMergeMode {
    match origin {
        // Native property bursts can still be mid-transition
        MediaRefreshOrigin::Bus => MediaCacheMergeMode::Transitioning,
        // Delayed retries reconcile sparse snapshots to their final state
        MediaRefreshOrigin::Fallback => MediaCacheMergeMode::Stable,
    }
}
