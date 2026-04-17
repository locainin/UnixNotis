use std::collections::HashMap;

use futures_util::StreamExt;
use tokio::sync::mpsc;
use tracing::warn;
use unixnotis_core::MediaConfig;
use zbus::fdo::DBusProxy;
use zbus::Connection;

use crate::dbus::UiEvent;

use super::media_bus::{
    build_player_state, handle_command, is_allowed_player, refresh_players,
    spawn_properties_listener, PlayerState,
};
use super::media_cache::{
    refresh_cache, refresh_player_cache, send_snapshot_if_changed, MediaCacheMergeMode,
};
use super::media_runtime::MEDIA_SIGNAL_CAPACITY;
use super::media_schedule::{
    cancel_delayed_refresh, prune_delayed_refreshes, schedule_command_refresh,
    schedule_metadata_fallback, schedule_metadata_fallbacks, DelayedRefreshTasks,
};
use super::{MediaCommand, MediaInfo, MediaRefreshOrigin, MediaSignal, MPRIS_PREFIX};

struct MediaRuntimeState {
    // Live player proxies keyed by bus name
    players: HashMap<String, PlayerState>,
    // Last known media snapshot per player
    cache: HashMap<String, MediaInfo>,
    // Last emitted snapshot lets the loop drop duplicate UI updates cheaply
    last_snapshot: Vec<MediaInfo>,
    // One delayed retry plan per player
    delayed_refreshes: DelayedRefreshTasks,
}

impl MediaRuntimeState {
    fn new() -> Self {
        // A fresh loop starts empty and fills from the first refresh pass
        Self {
            players: HashMap::new(),
            cache: HashMap::new(),
            last_snapshot: Vec::new(),
            delayed_refreshes: HashMap::new(),
        }
    }
}

pub(super) async fn run_media_loop(
    connection: Connection,
    config: MediaConfig,
    sender: async_channel::Sender<UiEvent>,
    mut command_rx: mpsc::Receiver<MediaCommand>,
) {
    let dbus_proxy = match DBusProxy::new(&connection).await {
        Ok(proxy) => proxy,
        Err(err) => {
            warn!(?err, "failed to create D-Bus proxy for media");
            return;
        }
    };

    let mut owner_stream = match dbus_proxy.receive_name_owner_changed().await {
        Ok(stream) => stream,
        Err(err) => {
            warn!(?err, "failed to subscribe to name owner changes");
            return;
        }
    };

    // This channel keeps property updates away from the GTK thread
    let (signal_tx, mut signal_rx) = mpsc::channel::<MediaSignal>(MEDIA_SIGNAL_CAPACITY);
    let mut state = MediaRuntimeState::new();
    // Startup begins with one full refresh so the UI gets a complete snapshot
    let mut refresh = true;

    loop {
        if refresh {
            // Full refresh rebuilds the visible player set from the bus
            refresh_all_players(
                &connection,
                &dbus_proxy,
                &config,
                &signal_tx,
                &mut state,
                &sender,
            )
            .await;
            refresh = false;
        }

        tokio::select! {
            command = command_rx.recv() => {
                let Some(command) = command else {
                    // Closing the command side shuts the media runtime down cleanly
                    break;
                };
                match command {
                    MediaCommand::Refresh => {
                        // Full refresh is used after startup and explicit reloads
                        refresh = true;
                    }
                    command => {
                        handle_runtime_command(
                            &mut state,
                            &signal_tx,
                            &sender,
                            command,
                        ).await;
                    }
                }
            }
            signal = signal_rx.recv() => {
                let Some(signal) = signal else {
                    // A closed signal channel means no more property updates can arrive
                    break;
                };
                handle_runtime_signal(
                    &mut state,
                    &signal_tx,
                    &sender,
                    signal,
                ).await;
            }
            signal = owner_stream.next() => {
                let Some(signal) = signal else {
                    // If the owner stream ends, the bus subscription is gone too
                    break;
                };
                if let Ok(args) = signal.args() {
                    // Name owner changes tell the loop when players appear or vanish
                    let name = args.name();
                    let new_owner = args
                        .new_owner()
                        .as_ref()
                        .map(|owner| owner.as_str().to_string());
                    if let Err(err) = apply_owner_change(
                        name,
                        new_owner.as_deref(),
                        &connection,
                        &config,
                        &signal_tx,
                        &mut state,
                        &sender,
                    )
                    .await
                    {
                        warn!(?err, "failed to apply media owner change");
                    }
                }
            }
        }
    }
}

async fn refresh_all_players(
    connection: &Connection,
    dbus_proxy: &DBusProxy<'_>,
    config: &MediaConfig,
    signal_tx: &mpsc::Sender<MediaSignal>,
    state: &mut MediaRuntimeState,
    sender: &async_channel::Sender<UiEvent>,
) {
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

async fn handle_runtime_command(
    state: &mut MediaRuntimeState,
    signal_tx: &mpsc::Sender<MediaSignal>,
    sender: &async_channel::Sender<UiEvent>,
    command: MediaCommand,
) {
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

async fn handle_runtime_signal(
    state: &mut MediaRuntimeState,
    signal_tx: &mpsc::Sender<MediaSignal>,
    sender: &async_channel::Sender<UiEvent>,
    signal: MediaSignal,
) {
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

async fn apply_owner_change(
    name: &str,
    new_owner: Option<&str>,
    connection: &Connection,
    config: &MediaConfig,
    signal_tx: &mpsc::Sender<MediaSignal>,
    state: &mut MediaRuntimeState,
    sender: &async_channel::Sender<UiEvent>,
) -> zbus::Result<()> {
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
        return Ok(());
    }

    if let Some(player_state) = build_player_state(connection, name, config).await? {
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
mod tests {
    use super::{
        merge_mode_for_signal, should_publish_immediate_command_snapshot,
        should_schedule_metadata_fallback, MediaCacheMergeMode,
    };
    use crate::media::{MediaCommand, MediaRefreshOrigin};

    #[test]
    fn fallback_generated_refreshes_do_not_rearm_followup_sweeps() {
        assert!(!should_schedule_metadata_fallback(
            MediaRefreshOrigin::Fallback
        ));
    }

    #[test]
    fn bus_generated_refreshes_still_allow_one_bounded_followup_sweep() {
        assert!(should_schedule_metadata_fallback(MediaRefreshOrigin::Bus));
    }

    #[test]
    fn skip_commands_wait_for_followup_refreshes() {
        assert!(!should_publish_immediate_command_snapshot(
            &MediaCommand::Next {
                bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
            }
        ));
        assert!(!should_publish_immediate_command_snapshot(
            &MediaCommand::Previous {
                bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
            }
        ));
    }

    #[test]
    fn play_pause_still_refreshes_immediately() {
        assert!(should_publish_immediate_command_snapshot(
            &MediaCommand::PlayPause {
                bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
            }
        ));
    }

    #[test]
    fn bus_updates_use_transition_merge_but_fallbacks_commit_final_state() {
        assert_eq!(
            merge_mode_for_signal(MediaRefreshOrigin::Bus),
            MediaCacheMergeMode::Transitioning
        );
        assert_eq!(
            merge_mode_for_signal(MediaRefreshOrigin::Fallback),
            MediaCacheMergeMode::Stable
        );
    }
}
