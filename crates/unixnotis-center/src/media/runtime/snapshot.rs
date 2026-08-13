use std::collections::HashMap;

use async_channel::Sender;
use tracing::debug;

use crate::control::UiEvent;

use crate::media::{mpris::is_plasma_browser_bridge, MediaInfo};

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
            info.bus_name.clone(),
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
    // A player can share one key with one group and another key with a second
    // group, so pairwise replacement is not enough. Build connected components
    // first, then choose one deterministic representative per component
    let mut parents = (0..infos.len()).collect::<Vec<_>>();
    let mut key_owner = HashMap::<String, usize>::new();
    for (index, info) in infos.iter().enumerate() {
        for key in dedupe_keys(info) {
            if let Some(previous) = key_owner.insert(key, index) {
                union(&mut parents, previous, index);
            }
        }
    }

    let mut representatives = HashMap::<usize, ComponentSelection>::new();
    for index in 0..infos.len() {
        let root = find(&mut parents, index);
        representatives
            .entry(root)
            .and_modify(|selection| {
                selection.first_index = selection.first_index.min(index);
                if representative_precedes(&infos[index], &infos[selection.representative_index]) {
                    selection.representative_index = index;
                }
            })
            .or_insert(ComponentSelection {
                first_index: index,
                representative_index: index,
            });
    }

    let mut selected = representatives.into_values().collect::<Vec<_>>();
    selected.sort_unstable_by_key(|selection| selection.first_index);
    selected
        .into_iter()
        .map(|selection| infos[selection.representative_index].clone())
        .collect()
}

struct ComponentSelection {
    // Preserve the first component position even when a later player is the best card
    first_index: usize,
    // Artwork and playback state choose the representative shown to the user
    representative_index: usize,
}

fn find(parents: &mut [usize], index: usize) -> usize {
    if parents[index] == index {
        return index;
    }
    let root = find(parents, parents[index]);
    parents[index] = root;
    root
}

fn union(parents: &mut [usize], left: usize, right: usize) {
    let left = find(parents, left);
    let right = find(parents, right);
    if left != right {
        parents[right] = left;
    }
}

fn representative_precedes(candidate: &MediaInfo, current: &MediaInfo) -> bool {
    media_score(candidate) < media_score(current)
        || (media_score(candidate) == media_score(current) && candidate.bus_name < current.bus_name)
}

fn dedupe_keys(info: &MediaInfo) -> Vec<String> {
    let has_browser_process_identity =
        info.browser_family.is_some() || info.source_pid_hint.is_some();
    if has_browser_process_identity {
        // A bridge helper owns several sessions, so its PID is not the browser identity
        if let Some(pid) = browser_process_pid(info) {
            return vec![format!("browser-process:{pid}")];
        }

        let title = info.title.trim();
        let artist = info.artist.trim();
        if !title.is_empty() && !artist.is_empty() {
            // Metadata is only a fallback when no process identity exists
            return vec![format!(
                "browser-track\n{}\n{}",
                normalize_token(title),
                normalize_token(artist),
            )];
        }
        // A family name alone is not a track identity
        return Vec::new();
    }
    let title = info.title.trim();
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

fn browser_process_pid(info: &MediaInfo) -> Option<u32> {
    if let Some(source_pid) = info.source_pid_hint {
        // kde:pid identifies the browser that supplied the bridge metadata
        return Some(source_pid);
    }
    if is_plasma_browser_bridge(&info.bus_name) {
        // The authenticated owner is only the shared bridge helper
        return None;
    }
    info.owner_pid
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
