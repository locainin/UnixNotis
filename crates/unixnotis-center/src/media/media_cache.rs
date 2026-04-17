use std::collections::HashMap;

use async_channel::Sender;
use tracing::debug;

use crate::dbus::UiEvent;

use super::media_bus::PlayerState;
use super::media_metadata::fetch_media_info;
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
    let previous = cache.clone();
    let mut next = HashMap::with_capacity(players.len());
    let states: Vec<PlayerState> = players.values().cloned().collect();
    for state in states {
        // A transient DBus read error should not blank a live player card
        // Keep the last good snapshot until a fresh read succeeds or the player disappears
        if let Some(info) = merge_media_info(
            previous.get(&state.bus_name),
            fetch_media_info(&state).await,
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
    let Some(state) = players.get(bus_name).cloned() else {
        cache.remove(bus_name);
        return;
    };
    if let Some(info) = merge_media_info(
        cache.get(bus_name),
        fetch_media_info(&state).await,
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

pub(super) async fn send_snapshot_if_changed(
    sender: &Sender<UiEvent>,
    cache: &HashMap<String, MediaInfo>,
    last_snapshot: &mut Vec<MediaInfo>,
) {
    // Snapshot keeps UI updates atomic and ordered.
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
            out.push(' ');
            last_space = true;
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dbus::UiEvent;
    use crate::media::MediaInfo;
    use tokio::runtime::Builder;

    fn make_info(
        bus_name: &str,
        identity: &str,
        playback_status: &str,
        has_art: bool,
        browser_family: Option<&str>,
    ) -> MediaInfo {
        MediaInfo {
            bus_name: bus_name.to_string(),
            identity: identity.to_string(),
            browser_family: browser_family.map(|family| family.to_string()),
            title: "title".to_string(),
            artist: "artist".to_string(),
            playback_status: playback_status.to_string(),
            art_source: has_art.then(|| {
                crate::media::MediaArtSource::LocalFile(std::path::PathBuf::from("/tmp/art.png"))
            }),
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
            make_info("org.mpris.MediaPlayer2.b", "Zeta", "Paused", false, None),
        );
        cache.insert(
            "org.mpris.MediaPlayer2.a".to_string(),
            make_info("org.mpris.MediaPlayer2.a", "Alpha", "Playing", false, None),
        );
        cache.insert(
            "org.mpris.MediaPlayer2.c".to_string(),
            make_info("org.mpris.MediaPlayer2.c", "Beta", "Playing", false, None),
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
                true,
                Some("firefox"),
            ),
        );
        cache.insert(
            "org.mpris.MediaPlayer2.firefox.instance".to_string(),
            make_info(
                "org.mpris.MediaPlayer2.firefox.instance",
                "Firefox",
                "Playing",
                false,
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

    #[test]
    fn merge_media_info_keeps_last_good_entry_when_fetch_fails() {
        let existing = make_info("org.mpris.MediaPlayer2.a", "Alpha", "Playing", true, None);
        let merged = merge_media_info(Some(&existing), None, MediaCacheMergeMode::Stable)
            .expect("merged info");

        assert_eq!(merged.playback_status, "Playing");
        assert!(merged.art_source.is_some());
    }

    #[test]
    fn merge_media_info_prefers_new_snapshot_when_fetch_succeeds() {
        let existing = make_info("org.mpris.MediaPlayer2.a", "Alpha", "Paused", false, None);
        let fetched = make_info("org.mpris.MediaPlayer2.a", "Alpha", "Playing", true, None);
        let merged = merge_media_info(Some(&existing), Some(fetched), MediaCacheMergeMode::Stable)
            .expect("merged info");

        assert_eq!(merged.playback_status, "Playing");
        assert!(merged.art_source.is_some());
    }

    #[test]
    fn transition_merge_keeps_prior_art_until_followup_refresh() {
        let existing = make_info("org.mpris.MediaPlayer2.a", "Alpha", "Playing", true, None);
        let fetched = make_info("org.mpris.MediaPlayer2.a", "Beta", "Playing", false, None);
        let merged = merge_media_info(
            Some(&existing),
            Some(fetched),
            MediaCacheMergeMode::Transitioning,
        )
        .expect("merged info");

        assert_eq!(merged.title, "title");
        assert!(merged.art_source.is_some());
    }

    #[test]
    fn transition_merge_keeps_prior_text_when_player_goes_blank_mid_swap() {
        let existing = make_info("org.mpris.MediaPlayer2.a", "Alpha", "Playing", true, None);
        let mut fetched = make_info("org.mpris.MediaPlayer2.a", "Alpha", "Playing", false, None);
        fetched.title.clear();
        fetched.artist.clear();
        let merged = merge_media_info(
            Some(&existing),
            Some(fetched),
            MediaCacheMergeMode::Transitioning,
        )
        .expect("merged info");

        assert_eq!(merged.title, existing.title);
        assert_eq!(merged.artist, existing.artist);
        assert!(merged.art_source.is_some());
    }

    #[test]
    fn stable_merge_allows_missing_art_to_clear_after_retry_window() {
        let existing = make_info("org.mpris.MediaPlayer2.a", "Alpha", "Playing", true, None);
        let fetched = make_info("org.mpris.MediaPlayer2.a", "Beta", "Playing", false, None);
        let merged = merge_media_info(Some(&existing), Some(fetched), MediaCacheMergeMode::Stable)
            .expect("merged info");

        assert!(merged.art_source.is_none());
    }

    #[test]
    fn transition_merge_does_not_keep_old_media_after_stop() {
        let existing = make_info("org.mpris.MediaPlayer2.a", "Alpha", "Playing", true, None);
        let mut fetched = make_info("org.mpris.MediaPlayer2.a", "Alpha", "Stopped", false, None);
        fetched.title.clear();
        fetched.artist.clear();
        let merged = merge_media_info(
            Some(&existing),
            Some(fetched),
            MediaCacheMergeMode::Transitioning,
        )
        .expect("merged info");

        assert!(merged.title.is_empty());
        assert!(merged.artist.is_empty());
        assert!(merged.art_source.is_none());
    }

    #[test]
    fn unchanged_snapshot_is_not_resent() {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        runtime.block_on(async {
            let (tx, rx) = async_channel::bounded(4);
            let mut cache = HashMap::new();
            let mut last_snapshot = Vec::new();
            cache.insert(
                "org.mpris.MediaPlayer2.a".to_string(),
                make_info("org.mpris.MediaPlayer2.a", "Alpha", "Playing", true, None),
            );

            send_snapshot_if_changed(&tx, &cache, &mut last_snapshot).await;
            match rx.recv().await.expect("first snapshot event") {
                UiEvent::MediaUpdated(snapshot) => assert_eq!(snapshot.len(), 1),
                other => panic!("unexpected first event: {other:?}"),
            }

            send_snapshot_if_changed(&tx, &cache, &mut last_snapshot).await;
            assert!(rx.is_empty());
        });
    }

    #[test]
    fn clearing_snapshot_only_emits_once_for_same_empty_state() {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        runtime.block_on(async {
            let (tx, rx) = async_channel::bounded(4);
            let cache = HashMap::new();
            let mut last_snapshot = vec![make_info(
                "org.mpris.MediaPlayer2.a",
                "Alpha",
                "Playing",
                true,
                None,
            )];

            send_snapshot_if_changed(&tx, &cache, &mut last_snapshot).await;
            assert!(matches!(
                rx.recv().await.expect("clear event"),
                UiEvent::MediaCleared
            ));

            send_snapshot_if_changed(&tx, &cache, &mut last_snapshot).await;
            assert!(rx.is_empty());
        });
    }
}
