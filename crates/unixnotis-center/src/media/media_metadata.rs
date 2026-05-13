use std::collections::HashMap;

use zbus::zvariant::OwnedValue;

use super::media_bus::PlayerState;
use super::media_policy::normalize_art_source;
use super::MediaInfo;

pub(super) async fn fetch_media_info(state: &PlayerState) -> Option<MediaInfo> {
    // Missing metadata should not drop the card; fall back to identity-only.
    let metadata: HashMap<String, OwnedValue> = state
        .player
        .get_property("Metadata")
        .await
        .unwrap_or_default();
    let title = metadata_string(&metadata, "xesam:title").unwrap_or_default();
    let artist = metadata_artist(&metadata).unwrap_or_default();
    // Metadata PID wins because browser bridges publish the real browser process there
    let owner_pid = metadata_pid(&metadata).or(state.owner_pid);
    let art_source = metadata_string(&metadata, "mpris:artUrl")
        .and_then(|value| normalize_art_source(&value, state.remote_art_allowed));

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

fn metadata_string(map: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    let value = map.get(key)?;
    let owned = value.try_clone().ok()?;
    String::try_from(owned).ok()
}

fn metadata_artist(map: &HashMap<String, OwnedValue>) -> Option<String> {
    let value = map.get("xesam:artist")?;
    let artists_value = value.try_clone().ok()?;
    if let Ok(artists) = Vec::<String>::try_from(artists_value) {
        return artists.into_iter().next();
    }
    let owned = value.try_clone().ok()?;
    if let Ok(artist) = String::try_from(owned) {
        if !artist.trim().is_empty() {
            return Some(artist);
        }
    }
    None
}

fn metadata_pid(map: &HashMap<String, OwnedValue>) -> Option<u32> {
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use zbus::zvariant::OwnedValue;

    use super::metadata_pid;

    #[test]
    fn metadata_pid_reads_unsigned_kde_pid() {
        let mut metadata = HashMap::new();
        metadata.insert("kde:pid".to_string(), OwnedValue::from(103_380_u32));

        assert_eq!(metadata_pid(&metadata), Some(103_380));
    }

    #[test]
    fn metadata_pid_rejects_negative_kde_pid() {
        let mut metadata = HashMap::new();
        metadata.insert("kde:pid".to_string(), OwnedValue::from(-1_i32));

        assert_eq!(metadata_pid(&metadata), None);
    }
}
