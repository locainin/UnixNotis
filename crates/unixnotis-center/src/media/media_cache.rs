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
    cache: &mut HashMap<String, MediaInfo>,
) {
    cache.clear();
    let states: Vec<PlayerState> = players.values().cloned().collect();
    for state in states {
        if let Some(info) = fetch_media_info(&state).await {
            cache.insert(state.bus_name.clone(), info);
        }
    }
}

pub(super) async fn refresh_player_cache(
    players: &HashMap<String, PlayerState>,
    cache: &mut HashMap<String, MediaInfo>,
    bus_name: &str,
) {
    let Some(state) = players.get(bus_name).cloned() else {
        cache.remove(bus_name);
        return;
    };
    if let Some(info) = fetch_media_info(&state).await {
        cache.insert(bus_name.to_string(), info);
    } else {
        cache.remove(bus_name);
    }
}

pub(super) async fn send_snapshot(
    sender: &Sender<UiEvent>,
    cache: &HashMap<String, MediaInfo>,
) {
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
    infos.sort_by(|left, right| {
        let left_rank = playback_rank(&left.playback_status);
        let right_rank = playback_rank(&right.playback_status);
        left_rank.cmp(&right_rank).then_with(|| {
            left.identity
                .to_lowercase()
                .cmp(&right.identity.to_lowercase())
        })
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
    if let Some(family) = browser_family(&info.identity, &info.bus_name) {
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

fn browser_family(identity: &str, bus_name: &str) -> Option<&'static str> {
    let bus_lower = bus_name.to_lowercase();
    if let Some(family) = browser_family_from_value(&bus_lower) {
        return Some(family);
    }
    let identity_lower = identity.to_lowercase();
    browser_family_from_value(&identity_lower)
}

fn browser_family_from_value(value: &str) -> Option<&'static str> {
    if value.contains("brave") {
        return Some("brave");
    }
    if value.contains("firefox") {
        return Some("firefox");
    }
    if value.contains("chromium") {
        return Some("chromium");
    }
    if value.contains("chrome") {
        return Some("chrome");
    }
    if value.contains("vivaldi") {
        return Some("vivaldi");
    }
    if value.contains("edge") {
        return Some("edge");
    }
    None
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
