//! Cache management and snapshot building for media players.
//!
//! Ensures UI updates are derived from consistent cached state.

use std::collections::HashMap;

use async_channel::Sender;
use tracing::debug;

use crate::dbus::UiEvent;

use super::media_bus::PlayerState;
use super::media_metadata::fetch_media_info;
use super::MediaInfo;

pub(super) async fn refresh_cache(
    players: &HashMap<String, PlayerState>,
    browser_tokens: &[String],
    cache: &mut HashMap<String, MediaInfo>,
) {
    cache.clear();
    let states: Vec<PlayerState> = players.values().cloned().collect();
    for state in states {
        if let Some(info) = fetch_media_info(&state).await {
            let info = with_browser_family(info, browser_tokens);
            cache.insert(state.bus_name.clone(), info);
        }
    }
}

pub(super) async fn refresh_player_cache(
    players: &HashMap<String, PlayerState>,
    browser_tokens: &[String],
    cache: &mut HashMap<String, MediaInfo>,
    bus_name: &str,
) {
    let Some(state) = players.get(bus_name).cloned() else {
        cache.remove(bus_name);
        return;
    };
    if let Some(info) = fetch_media_info(&state).await {
        let info = with_browser_family(info, browser_tokens);
        cache.insert(bus_name.to_string(), info);
    } else {
        cache.remove(bus_name);
    }
}

pub(super) async fn send_snapshot(sender: &Sender<UiEvent>, cache: &HashMap<String, MediaInfo>) {
    // Snapshot keeps UI updates atomic and ordered.
    let snapshot = build_snapshot(cache);
    if snapshot.is_empty() {
        let _ = sender.send(UiEvent::MediaCleared).await;
    } else {
        let _ = sender.send(UiEvent::MediaUpdated(snapshot)).await;
    }
}

fn build_snapshot(cache: &HashMap<String, MediaInfo>) -> Vec<MediaInfo> {
    let mut infos: Vec<MediaInfo> = cache
        .values()
        .filter(|info| is_active_player(info))
        .cloned()
        .collect();
    let original_len = infos.len();
    // Cache sort keys to avoid repeated lowercasing in the comparator.
    infos.sort_by_cached_key(|info| {
        (
            playback_rank(&info.playback_status),
            info.identity.to_lowercase(),
        )
    });
    let deduped = dedupe_players(infos);
    if deduped.len() != original_len {
        debug!(
            original = original_len,
            deduped = deduped.len(),
            "deduped media players"
        );
    }
    deduped
}

fn playback_rank(status: &str) -> u8 {
    match status {
        "Playing" => 0,
        "Paused" => 1,
        _ => 2,
    }
}

fn is_active_player(info: &MediaInfo) -> bool {
    // Playing and paused sessions remain visible to avoid disappearing on pause.
    matches!(info.playback_status.as_str(), "Playing" | "Paused")
}

fn dedupe_players(infos: Vec<MediaInfo>) -> Vec<MediaInfo> {
    let mut output: Vec<MediaInfo> = Vec::with_capacity(infos.len());
    let mut seen: HashMap<String, usize> = HashMap::new();
    for info in infos {
        let Some(key) = dedupe_key(&info) else {
            output.push(info);
            continue;
        };
        if let Some(existing_index) = seen.get(&key).copied() {
            let existing = &output[existing_index];
            if media_score(&info) < media_score(existing) {
                output[existing_index] = info;
            }
            continue;
        }
        seen.insert(key, output.len());
        output.push(info);
    }
    output
}

fn dedupe_key(info: &MediaInfo) -> Option<String> {
    if let Some(family) = info.browser_family.as_deref() {
        return Some(format!("browser:{family}"));
    }
    let title = info.title.trim();
    if title.is_empty() {
        return None;
    }
    let artist = info.artist.trim();
    let identity = info.identity.trim();
    let normalized_title = normalize_token(title);
    let normalized_artist = normalize_token(artist);
    Some(format!(
        "{}\n{}\n{}",
        normalize_token(identity),
        normalized_title,
        normalized_artist
    ))
}

fn media_score(info: &MediaInfo) -> (u8, u8) {
    let status = playback_rank(&info.playback_status);
    let art_rank = if info.art_uri.is_some() { 0 } else { 1 };
    (status, art_rank)
}

fn with_browser_family(mut info: MediaInfo, browser_tokens: &[String]) -> MediaInfo {
    // Browser tokens are config-driven to avoid hardcoded identification lists.
    info.browser_family = detect_browser_family(&info.identity, &info.bus_name, browser_tokens);
    info
}

fn detect_browser_family(
    identity: &str,
    bus_name: &str,
    browser_tokens: &[String],
) -> Option<String> {
    if browser_tokens.is_empty() {
        return None;
    }
    let bus_lower = bus_name.to_lowercase();
    if let Some(family) = browser_family_from_value(&bus_lower, browser_tokens) {
        return Some(family);
    }
    let identity_lower = identity.to_lowercase();
    browser_family_from_value(&identity_lower, browser_tokens).or_else(|| {
        // Fall back to MPRIS suffix when identity signals a browser but tokens miss.
        if !identity_lower.contains("browser") {
            return None;
        }
        mpris_suffix(&bus_lower).map(|suffix| suffix.to_string())
    })
}

fn browser_family_from_value(value: &str, browser_tokens: &[String]) -> Option<String> {
    for token in browser_tokens {
        if value.contains(token) {
            return Some(token.clone());
        }
    }
    None
}

fn mpris_suffix(bus_name: &str) -> Option<&str> {
    let suffix = bus_name.strip_prefix("org.mpris.mediaplayer2.")?;
    // Keep only the first segment to group instance-specific bus names together.
    Some(suffix.split('.').next().unwrap_or(suffix))
}

fn normalize_token(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut last_space = false;
    for ch in value.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            last_space = false;
            continue;
        }
        if lower.is_whitespace() && !last_space {
            out.push(' ');
            last_space = true;
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::MediaInfo;

    fn make_info(
        bus_name: &str,
        identity: &str,
        playback_status: &str,
        art_uri: Option<&str>,
        browser_family: Option<&str>,
    ) -> MediaInfo {
        MediaInfo {
            bus_name: bus_name.to_string(),
            identity: identity.to_string(),
            browser_family: browser_family.map(|family| family.to_string()),
            title: "title".to_string(),
            artist: "artist".to_string(),
            playback_status: playback_status.to_string(),
            art_uri: art_uri.map(|uri| uri.to_string()),
            can_play: true,
            can_pause: true,
            can_next: true,
            can_prev: true,
        }
    }

    #[test]
    fn build_snapshot_sorts_by_status_then_identity() {
        let mut cache = HashMap::new();
        cache.insert(
            "org.mpris.MediaPlayer2.b".to_string(),
            make_info("org.mpris.MediaPlayer2.b", "Zeta", "Paused", None, None),
        );
        cache.insert(
            "org.mpris.MediaPlayer2.a".to_string(),
            make_info("org.mpris.MediaPlayer2.a", "Alpha", "Playing", None, None),
        );
        cache.insert(
            "org.mpris.MediaPlayer2.c".to_string(),
            make_info("org.mpris.MediaPlayer2.c", "Beta", "Playing", None, None),
        );

        let snapshot = build_snapshot(&cache);
        let identities: Vec<_> = snapshot.iter().map(|info| info.identity.as_str()).collect();
        assert_eq!(identities, vec!["Alpha", "Beta", "Zeta"]);
    }

    #[test]
    fn build_snapshot_dedupes_browser_family_by_score() {
        let mut cache = HashMap::new();
        cache.insert(
            "org.mpris.MediaPlayer2.firefox".to_string(),
            make_info(
                "org.mpris.MediaPlayer2.firefox",
                "Firefox",
                "Paused",
                Some("art://paused"),
                Some("firefox"),
            ),
        );
        cache.insert(
            "org.mpris.MediaPlayer2.firefox.instance".to_string(),
            make_info(
                "org.mpris.MediaPlayer2.firefox.instance",
                "Firefox",
                "Playing",
                None,
                Some("firefox"),
            ),
        );

        let snapshot = build_snapshot(&cache);
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].playback_status, "Playing");
    }

    #[test]
    fn normalize_token_compacts_and_lowercases() {
        let token = normalize_token("  Foo--Bar\tBaz  ");
        // Hyphens are treated as punctuation; only whitespace yields word boundaries.
        assert_eq!(token, "foobar baz");
    }
}
