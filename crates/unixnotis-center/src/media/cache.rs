use std::collections::HashMap;

use super::bus::PlayerState;
use super::metadata::fetch_media_info;
use super::MediaInfo;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MediaCacheMergeMode {
    // Startup, fallback, and full refresh paths should show the real current snapshot
    Stable,
    // Command and bus bursts can publish partial metadata for a moment during track swaps
    Transitioning,
}

pub(super) async fn refresh_cache(
    players: &HashMap<String, PlayerState>,
    cache: &mut HashMap<String, MediaInfo>,
) {
    // Move the old cache out so the merge path can reuse prior snapshots
    // without cloning the whole map on every refresh
    let previous = std::mem::take(cache);
    let mut next = HashMap::with_capacity(players.len());
    for state in players.values() {
        // A transient DBus read error should not blank a live player card
        // Keep the last good snapshot until a fresh read succeeds or the player disappears
        if let Some(info) = merge_media_info(
            previous.get(&state.bus_name),
            fetch_media_info(state).await,
            MediaCacheMergeMode::Stable,
        ) {
            next.insert(state.bus_name.clone(), info);
        }
    }
    *cache = next;
}

pub(super) async fn refresh_player_cache(
    players: &HashMap<String, PlayerState>,
    cache: &mut HashMap<String, MediaInfo>,
    bus_name: &str,
    merge_mode: MediaCacheMergeMode,
) {
    let Some(state) = players.get(bus_name) else {
        cache.remove(bus_name);
        return;
    };
    if let Some(info) = merge_media_info(
        cache.get(bus_name),
        fetch_media_info(state).await,
        merge_mode,
    ) {
        cache.insert(bus_name.to_string(), info);
    }
}

fn merge_media_info(
    existing: Option<&MediaInfo>,
    fetched: Option<MediaInfo>,
    merge_mode: MediaCacheMergeMode,
) -> Option<MediaInfo> {
    let Some(fetched) = fetched else {
        return existing.cloned();
    };
    let Some(existing) = existing else {
        return Some(fetched);
    };

    match merge_mode {
        MediaCacheMergeMode::Stable => Some(fetched),
        MediaCacheMergeMode::Transitioning => Some(preserve_transition_fields(existing, fetched)),
    }
}

fn preserve_transition_fields(existing: &MediaInfo, mut fetched: MediaInfo) -> MediaInfo {
    if !is_live_player(&fetched) {
        // Stopped or idle snapshots should be able to clear old media content right away
        return fetched;
    }

    if fetched.art_source.is_none() && existing.art_source.is_some() {
        // Track changes often lose the art url for one update before the late image arrives
        fetched.art_source = existing.art_source.clone();
    }

    if metadata_is_blank(&fetched) && metadata_has_content(existing) {
        // A blank transition frame is worse than holding the prior text for one retry window
        fetched.title = existing.title.clone();
        fetched.artist = existing.artist.clone();
    }

    fetched
}

fn metadata_is_blank(info: &MediaInfo) -> bool {
    info.title.trim().is_empty() && info.artist.trim().is_empty()
}

fn metadata_has_content(info: &MediaInfo) -> bool {
    !metadata_is_blank(info)
}

fn is_live_player(info: &MediaInfo) -> bool {
    matches!(info.playback_status.as_str(), "Playing" | "Paused")
}

#[cfg(test)]
#[path = "tests/cache.rs"]
mod tests;
