use std::collections::HashMap;

use zbus::zvariant::OwnedValue;

use super::constants::{
    MAX_METADATA_ENTRIES, MAX_MPRIS_PROPERTY_REPLY_BYTES, MPRIS_PROPERTY_TIMEOUT_MS,
};
use super::player::PlayerState;
use crate::media::art::normalize_art_source;
use crate::media::MediaInfo;
use zbus::Proxy;

// Bound MPRIS metadata fields before copying into runtime snapshots
const MAX_TITLE_BYTES: usize = 256;
const MAX_ARTIST_BYTES: usize = 256;
const MAX_ART_URL_BYTES: usize = 2048;

pub(in crate::media) async fn fetch_media_info(state: &PlayerState) -> Option<MediaInfo> {
    if state.timeout.is_quarantined() {
        return None;
    }
    let timeout = std::time::Duration::from_millis(MPRIS_PROPERTY_TIMEOUT_MS);
    let (metadata, playback_status, can_play, can_pause, can_next, can_prev) = tokio::join!(
        bounded_property::<HashMap<String, OwnedValue>>(&state.property_calls, "Metadata", timeout,),
        bounded_property::<String>(&state.property_calls, "PlaybackStatus", timeout),
        bounded_property::<bool>(&state.property_calls, "CanPlay", timeout),
        bounded_property::<bool>(&state.property_calls, "CanPause", timeout),
        bounded_property::<bool>(&state.property_calls, "CanGoNext", timeout),
        bounded_property::<bool>(&state.property_calls, "CanGoPrevious", timeout),
    );
    let metadata = metadata
        .filter(|map| metadata_entry_count_allowed(map.len()))
        .unwrap_or_default();
    let title = metadata_string(&metadata, "xesam:title")
        .map(|value| bound_string(&value, MAX_TITLE_BYTES))
        .unwrap_or_default();
    let artist = metadata_artist(&metadata)
        .map(|value| bound_string(&value, MAX_ARTIST_BYTES))
        .unwrap_or_default();
    let art_source = metadata_string(&metadata, "mpris:artUrl")
        .filter(|value| value.len() <= MAX_ART_URL_BYTES)
        .and_then(|value| {
            normalize_art_source(&value, state.remote_art_allowed, state.local_art_allowed)
        });

    // PlaybackStatus drives whether the player stays visible
    // A missing status keeps the prior cache entry instead of inventing a stop event
    let Some(playback_status) = playback_status else {
        state.timeout.record_timeout();
        return None;
    };
    state.timeout.clear_timeout();
    let can_play = can_play.unwrap_or(false);
    let can_pause = can_pause.unwrap_or(false);
    let can_next = can_next.unwrap_or(false);
    let can_prev = can_prev.unwrap_or(false);

    Some(MediaInfo {
        bus_name: state.bus_name.clone(),
        identity: state.identity.clone(),
        // Browser family is decided once when the player is admitted.
        browser_family: state.browser_family.clone(),
        // Metadata PIDs are caller-controlled hints and never replace bus credentials
        owner_pid: state.owner_pid,
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

/// Check the raw reply body before asking zvariant to allocate dynamic values
async fn bounded_property<T>(
    proxy: &Proxy<'static>,
    property: &str,
    timeout: std::time::Duration,
) -> Option<T>
where
    T: TryFrom<OwnedValue>,
{
    let reply = tokio::time::timeout(
        timeout,
        proxy.call_method("Get", &(super::constants::MPRIS_PLAYER, property)),
    )
    .await
    .ok()?
    .ok()?;
    if !property_reply_body_allowed(reply.body().len()) {
        return None;
    }
    let value: OwnedValue = reply.body().deserialize().ok()?;
    T::try_from(value).ok()
}

pub(super) const fn metadata_entry_count_allowed(count: usize) -> bool {
    count <= MAX_METADATA_ENTRIES
}

pub(super) const fn property_reply_body_allowed(body_len: usize) -> bool {
    body_len <= MAX_MPRIS_PROPERTY_REPLY_BYTES
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
