use std::collections::HashMap;

use zbus::zvariant::OwnedValue;

use super::PlayerState;
use crate::media::art::normalize_art_source;
use crate::media::MediaInfo;

// Bound MPRIS metadata fields before copying into runtime snapshots
const MAX_TITLE_BYTES: usize = 256;
const MAX_ARTIST_BYTES: usize = 256;
const MAX_ART_URL_BYTES: usize = 2048;

pub(in crate::media) async fn fetch_media_info(state: &PlayerState) -> Option<MediaInfo> {
    // Missing metadata should not drop the card; fall back to identity-only.
    let metadata: HashMap<String, OwnedValue> = state
        .player
        .get_property("Metadata")
        .await
        .unwrap_or_default();
    let title = metadata_string(&metadata, "xesam:title")
        .map(|value| bound_string(&value, MAX_TITLE_BYTES))
        .unwrap_or_default();
    let artist = metadata_artist(&metadata)
        .map(|value| bound_string(&value, MAX_ARTIST_BYTES))
        .unwrap_or_default();
    // Metadata PID wins because browser bridges publish the real browser process there
    let owner_pid = metadata_pid(&metadata).or(state.owner_pid);
    let art_source = metadata_string(&metadata, "mpris:artUrl")
        .filter(|value| value.len() <= MAX_ART_URL_BYTES)
        .and_then(|value| normalize_art_source(&value, state.remote_art_allowed, state.local_art_allowed));

    // PlaybackStatus drives whether the player stays visible
    // If that read fails, keep the previous snapshot instead of inventing a fake stop event
    let playback_status: String = state.player.get_property("PlaybackStatus").await.ok()?;
    let can_play: bool = state.player.get_property("CanPlay").await.unwrap_or(false);
    let can_pause: bool = state.player.get_property("CanPause").await.unwrap_or(false);
    let can_next: bool = state
        .player
        .get_property("CanGoNext")
        .await
        .unwrap_or(false);
    let can_prev: bool = state
        .player
        .get_property("CanGoPrevious")
        .await
        .unwrap_or(false);

    Some(MediaInfo {
        bus_name: state.bus_name.clone(),
        identity: state.identity.clone(),
        // Browser family is decided once when the player is admitted.
        browser_family: state.browser_family.clone(),
        // Plasma browser integration reports the real browser PID as kde:pid
        // That PID is stronger than the bridge process owner for duplicate checks
        owner_pid,
        title,
        artist,
        playback_status,
        art_source,
        can_play,
        can_pause,
        can_next,
        can_prev,
    })
}

fn bound_string(value: &str, max_bytes: usize) -> String {
    // Truncate at a UTF-8 boundary so the retained value stays valid
    let trimmed = value.trim();
    if trimmed.len() <= max_bytes {
        return trimmed.to_string();
    }
    let mut end = max_bytes;
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    trimmed[..end].to_string()
}

fn metadata_string(map: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    let value = map.get(key)?;
    let owned = value.try_clone().ok()?;
    String::try_from(owned).ok()
}

fn metadata_artist(map: &HashMap<String, OwnedValue>) -> Option<String> {
    let value = map.get("xesam:artist")?;
    let artists_value = value.try_clone().ok()?;
    if let Ok(artists) = Vec::<String>::try_from(artists_value) {
        // Bound the number of artist entries before taking the first one
        if artists.len() > 16 {
            return None;
        }
        return artists
            .into_iter()
            .next()
            .filter(|artist| !artist.trim().is_empty());
    }
    let owned = value.try_clone().ok()?;
    if let Ok(artist) = String::try_from(owned) {
        if !artist.trim().is_empty() {
            return Some(artist);
        }
    }
    None
}

pub(super) fn metadata_pid(map: &HashMap<String, OwnedValue>) -> Option<u32> {
    let value = map.get("kde:pid")?;
    // KDE currently sends this as an integer PID, but bindings may expose signed values
    let owned = value.try_clone().ok()?;
    if let Ok(pid) = i32::try_from(owned) {
        return u32::try_from(pid).ok();
    }
    // Accept unsigned variants too so callers do not depend on one zvariant shape
    let owned = value.try_clone().ok()?;
    if let Ok(pid) = u32::try_from(owned) {
        return Some(pid);
    }
    None
}
