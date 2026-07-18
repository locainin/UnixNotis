//! Full player discovery and cache refreshes

use std::collections::HashMap;

use tokio::sync::mpsc;
use tracing::warn;
use unixnotis_core::MediaConfig;
use zbus::fdo::DBusProxy;
use zbus::Connection;

use super::cache::refresh_cache;
use super::schedule::{prune_delayed_refreshes, schedule_metadata_fallbacks, DelayedRefreshTasks};
use super::snapshot::send_snapshot_if_changed;
use super::state::MediaRuntimeState;
use super::MediaSignal;
use crate::control::UiEvent;
use crate::media::mpris::{refresh_players, PlayerState};

pub(super) async fn refresh_all_players(
    connection: &Connection,
    dbus_proxy: &DBusProxy<'_>,
    config: &MediaConfig,
    signal_tx: &mpsc::Sender<MediaSignal>,
    state: &mut MediaRuntimeState,
    sender: &async_channel::Sender<UiEvent>,
) {
    // Full refresh owns the current player inventory for this bus generation
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
    // Cache rebuild happens after discovery so the snapshot stays aligned
    refresh_cache(&state.players, &mut state.cache).await;
    send_snapshot_if_changed(sender, &state.cache, &mut state.last_snapshot).await;
    schedule_metadata_fallbacks(
        &mut state.delayed_refreshes,
        &state.cache,
        signal_tx.clone(),
    );
}

pub(super) fn prune_player_refreshes(
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
