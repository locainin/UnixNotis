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
const PLASMA_BRIDGE: &str = "org.mpris.MediaPlayer2.plasma-browser-integration";

#[derive(Debug)]
pub(super) enum PropertyRead<T> {
    Value(T),
    Timeout,
    Oversize,
    Invalid,
    BusError,
}

impl<T> PropertyRead<T> {
    pub(super) const fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout)
    }

    pub(super) fn into_value(self) -> Option<T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Timeout | Self::Oversize | Self::Invalid | Self::BusError => None,
        }
    }
}

pub(in crate::media) async fn fetch_media_info(state: &PlayerState) -> Option<MediaInfo> {
    if state.timeout.is_quarantined() {
        return None;
    }
    let timeout = std::time::Duration::from_millis(MPRIS_PROPERTY_TIMEOUT_MS);
    let (metadata, playback_status, can_play, can_pause, can_next, can_prev) = tokio::join!(
        bounded_property::<HashMap<String, OwnedValue>>(
            &state.property_calls,
            super::constants::MPRIS_PLAYER,
            "Metadata",
            timeout,
        ),
        bounded_property::<String>(
            &state.property_calls,
            super::constants::MPRIS_PLAYER,
            "PlaybackStatus",
            timeout,
        ),
        bounded_property::<bool>(
            &state.property_calls,
            super::constants::MPRIS_PLAYER,
            "CanPlay",
            timeout
        ),
        bounded_property::<bool>(
            &state.property_calls,
            super::constants::MPRIS_PLAYER,
            "CanPause",
            timeout
        ),
        bounded_property::<bool>(
            &state.property_calls,
            super::constants::MPRIS_PLAYER,
            "CanGoNext",
            timeout
        ),
        bounded_property::<bool>(
            &state.property_calls,
            super::constants::MPRIS_PLAYER,
            "CanGoPrevious",
            timeout
        ),
    );
    // Timeout quarantine is a refresh-batch invariant. A fast PlaybackStatus
    // response must not erase timeouts from other calls in the same refresh
    let any_timeout = metadata.is_timeout()
        || playback_status.is_timeout()
        || can_play.is_timeout()
        || can_pause.is_timeout()
        || can_next.is_timeout()
        || can_prev.is_timeout();
    state.timeout.record_refresh_batch(any_timeout);

    let metadata = metadata
        .into_value()
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
    // Only the Plasma bridge contract defines kde:pid as a source-browser hint
    let source_pid_hint = is_plasma_browser_bridge(&state.bus_name)
        .then(|| metadata_pid(&metadata, "kde:pid"))
        .flatten();

    // PlaybackStatus drives whether the player stays visible
    // A missing status keeps the prior cache entry instead of inventing a stop event
    let playback_status = playback_status.into_value()?;
    let can_play = can_play.into_value().unwrap_or(false);
    let can_pause = can_pause.into_value().unwrap_or(false);
    let can_next = can_next.into_value().unwrap_or(false);
    let can_prev = can_prev.into_value().unwrap_or(false);

    Some(MediaInfo {
        bus_name: state.bus_name.clone(),
        identity: state.identity.clone(),
        // Browser family is decided once when the player is admitted.
        browser_family: state.browser_family.clone(),
        // The broker PID remains the authority for process-bound policy
        owner_pid: state.owner_pid,
        // KDE bridge metadata is retained separately and used only for deduplication
        source_pid_hint,
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
pub(super) async fn bounded_property<T>(
    proxy: &Proxy<'static>,
    interface: &str,
    property: &str,
    timeout: std::time::Duration,
) -> PropertyRead<T>
where
    T: TryFrom<OwnedValue>,
{
    let reply =
        match tokio::time::timeout(timeout, proxy.call_method("Get", &(interface, property))).await
        {
            Err(_elapsed) => return PropertyRead::Timeout,
            Ok(Err(_error)) => return PropertyRead::BusError,
            Ok(Ok(reply)) => reply,
        };
    if !property_reply_body_allowed(reply.body().len()) {
        return PropertyRead::Oversize;
    }
    let Ok(value) = reply.body().deserialize::<OwnedValue>() else {
        return PropertyRead::Invalid;
    };
    match T::try_from(value) {
        Ok(value) => PropertyRead::Value(value),
        Err(_error) => PropertyRead::Invalid,
    }
}

pub(super) const fn metadata_entry_count_allowed(count: usize) -> bool {
    count <= MAX_METADATA_ENTRIES
}

pub(super) const fn property_reply_body_allowed(body_len: usize) -> bool {
    body_len <= MAX_MPRIS_PROPERTY_REPLY_BYTES
}

pub(super) fn bound_string(value: &str, max_bytes: usize) -> String {
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

pub(super) fn metadata_string(map: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    let value = map.get(key)?;
    let owned = value.try_clone().ok()?;
    String::try_from(owned).ok()
}

pub(in crate::media) fn is_plasma_browser_bridge(bus_name: &str) -> bool {
    // Only the known bridge name may contribute an untrusted source-PID hint
    bus_name == PLASMA_BRIDGE
        || bus_name
            .strip_prefix(PLASMA_BRIDGE)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

pub(super) fn metadata_pid(map: &HashMap<String, OwnedValue>, key: &str) -> Option<u32> {
    // Zero and negative values do not identify a live process
    let value = map.get(key)?;
    let owned = value.try_clone().ok()?;
    if let Ok(pid) = i32::try_from(owned) {
        return u32::try_from(pid).ok().filter(|pid| *pid != 0);
    }
    let owned = value.try_clone().ok()?;
    u32::try_from(owned).ok().filter(|pid| *pid != 0)
}

pub(super) fn metadata_artist(map: &HashMap<String, OwnedValue>) -> Option<String> {
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
