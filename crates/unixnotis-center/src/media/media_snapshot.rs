use std::collections::HashMap;

use async_channel::Sender;
use tracing::debug;

use crate::dbus::UiEvent;

use super::MediaInfo;

pub(super) async fn send_snapshot_if_changed(
    sender: &Sender<UiEvent>,
    cache: &HashMap<String, MediaInfo>,
    last_snapshot: &mut Vec<MediaInfo>,
) {
    // Snapshot keeps UI updates atomic and ordered
    let snapshot = build_snapshot(cache);
    if *last_snapshot == snapshot {
        // Identical snapshots do not need another UI event or list rebuild path
        return;
    }
    *last_snapshot = snapshot.clone();
    if snapshot.is_empty() {
        if let Err(err) = sender.send(UiEvent::MediaCleared).await {
            // Closed UI channels are normal during teardown, but the drop should stay visible
            debug!(?err, "failed to send media cleared snapshot");
        }
    } else if let Err(err) = sender.send(UiEvent::MediaUpdated(snapshot)).await {
        // Lost snapshot sends leave the media view stale, so keep a debug breadcrumb here
        debug!(?err, "failed to send media updated snapshot");
    }
}

fn build_snapshot(cache: &HashMap<String, MediaInfo>) -> Vec<MediaInfo> {
    // Snapshot building is the last step before UI fanout
    // Keep filtering, sort order, and dedupe rules together so one pass defines
    // exactly what the panel sees
    let mut infos: Vec<MediaInfo> = cache
        .values()
        .filter(|info| is_active_player(info))
        .cloned()
        .collect();
    let original_len = infos.len();
    // Cache sort keys to avoid repeated lowercasing in the comparator
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
    // Playing and paused sessions remain visible to avoid disappearing on pause
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
            // Lower score wins, so a playing player with art beats a paused
            // or artless duplicate from the same browser family or track key
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
    let title = info.title.trim();
    if let Some(family) = info.browser_family.as_deref() {
        if let Some(pid) = info.owner_pid {
            // Browser bridges can publish the same tab under different MPRIS names
            // The source PID is the strongest signal that both cards mirror one source
            return Some(format!("browser-pid:{pid}"));
        }
        if !title.is_empty() {
            // Browser-backed players can expose one webpage through multiple MPRIS names
            // Track metadata is the stable key across Brave, Chromium, and browser instances
            let artist = info.artist.trim();
            return Some(format!(
                "browser-track\n{}\n{}",
                normalize_token(title),
                normalize_token(artist)
            ));
        }
        // Empty browser metadata is too weak for cross-browser matching
        // Keep the old family fallback so duplicate instances still collapse
        return Some(format!("browser:{family}"));
    }
    if title.is_empty() {
        // Empty titles are too weak to build a stable cross-player key
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
    // Duplicate groups keep the most useful card for the panel
    // Playing state matters first, then artwork breaks otherwise equal entries
    let status = playback_rank(&info.playback_status);
    let art_rank = if info.art_source.is_some() { 0 } else { 1 };
    (status, art_rank)
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
            // Collapse runs of whitespace into one separator so cosmetic spacing
            // differences do not break dedupe keys
            out.push(' ');
            last_space = true;
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
#[path = "tests/snapshot.rs"]
mod tests;
