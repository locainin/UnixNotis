use std::collections::HashMap;

use async_channel::Sender;
use tracing::debug;

use crate::control::UiEvent;

use crate::media::MediaInfo;

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
    last_snapshot.clone_from(&snapshot);
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

pub(super) fn build_snapshot(cache: &HashMap<String, MediaInfo>) -> Vec<MediaInfo> {
    // Snapshot building is the last step before UI fanout
    // Keep filtering, sort order, and dedupe rules together so one pass defines
    // exactly what the panel sees
    let mut infos: Vec<MediaInfo> = cache
        .values()
        .filter(|info| is_active_player(info))
        .cloned()
        .collect();
    // Cache sort keys to avoid repeated lowercasing in the comparator
    infos.sort_by_cached_key(|info| {
        (
            playback_rank(&info.playback_status),
            info.identity.to_lowercase(),
        )
    });
    dedupe_players(infos)
}

fn playback_rank(status: &str) -> u8 {
    u8::from(status != "Playing")
}

fn is_active_player(info: &MediaInfo) -> bool {
    // Playing and paused sessions remain visible to avoid disappearing on pause
    matches!(info.playback_status.as_str(), "Playing" | "Paused")
}

fn dedupe_players(infos: Vec<MediaInfo>) -> Vec<MediaInfo> {
    let mut output: Vec<MediaInfo> = Vec::with_capacity(infos.len());
    let mut seen: HashMap<String, usize> = HashMap::new();
    for info in infos {
        let keys = dedupe_keys(&info);
        if keys.is_empty() {
            output.push(info);
            continue;
        }
        if let Some(existing_index) = keys.iter().find_map(|key| seen.get(key).copied()) {
            let existing = &output[existing_index];
            // Lower score wins, so a playing player with art beats a paused
            // or artless duplicate from the same browser family or track key
            if media_score(&info) < media_score(existing) {
                output[existing_index] = info;
            }
            for key in keys {
                seen.insert(key, existing_index);
            }
            continue;
        }
        let output_index = output.len();
        for key in keys {
            seen.insert(key, output_index);
        }
        output.push(info);
    }
    output
}

fn dedupe_keys(info: &MediaInfo) -> Vec<String> {
    let title = info.title.trim();
    if let Some(family) = info.browser_family.as_deref() {
        let mut keys = Vec::with_capacity(2);
        if !title.is_empty() {
            // Browser bridges often expose the same track under different names and PIDs
            // Track identity is the useful cross-browser key when both title and artist exist
            let artist = info.artist.trim();
            keys.push(format!(
                "browser-track\n{}\n{}",
                normalize_token(title),
                normalize_token(artist),
            ));
        }
        if let Some(pid) = info.owner_pid {
            // A broker-derived PID also collapses aliases owned by one browser process
            keys.push(format!("browser-pid:{pid}"));
        }
        if keys.is_empty() {
            // Empty browser metadata is too weak for cross-browser matching
            // Keep the family fallback so duplicate instances still collapse
            keys.push(format!("browser:{family}"));
        }
        return keys;
    }
    if title.is_empty() {
        // Empty titles are too weak to build a stable cross-player key
        return Vec::new();
    }
    let artist = info.artist.trim();
    let identity = info.identity.trim();
    let normalized_title = normalize_token(title);
    let normalized_artist = normalize_token(artist);
    vec![format!(
        "{}\n{}\n{}",
        normalize_token(identity),
        normalized_title,
        normalized_artist
    )]
}

fn media_score(info: &MediaInfo) -> (u8, u8) {
    // Duplicate groups keep the most useful card for the panel
    // Playing state matters first, then artwork breaks otherwise equal entries
    let status = playback_rank(&info.playback_status);
    let art_rank = u8::from(info.art_source.is_none());
    (status, art_rank)
}

pub(super) fn normalize_token(value: &str) -> String {
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
