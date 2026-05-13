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
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use tokio::runtime::Builder;

    use super::{build_snapshot, normalize_token, send_snapshot_if_changed};
    use crate::dbus::UiEvent;
    use crate::media::{MediaArtSource, MediaInfo};

    fn make_info(
        bus_name: &str,
        identity: &str,
        playback_status: &str,
        has_art: bool,
        browser_family: Option<&str>,
        owner_pid: Option<u32>,
    ) -> MediaInfo {
        MediaInfo {
            bus_name: bus_name.to_string(),
            identity: identity.to_string(),
            browser_family: browser_family.map(|family| family.to_string()),
            owner_pid,
            title: "title".to_string(),
            artist: "artist".to_string(),
            playback_status: playback_status.to_string(),
            art_source: has_art.then(|| MediaArtSource::LocalFile(PathBuf::from("/tmp/art.png"))),
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
            make_info(
                "org.mpris.MediaPlayer2.b",
                "Zeta",
                "Paused",
                false,
                None,
                None,
            ),
        );
        cache.insert(
            "org.mpris.MediaPlayer2.a".to_string(),
            make_info(
                "org.mpris.MediaPlayer2.a",
                "Alpha",
                "Playing",
                false,
                None,
                None,
            ),
        );
        cache.insert(
            "org.mpris.MediaPlayer2.c".to_string(),
            make_info(
                "org.mpris.MediaPlayer2.c",
                "Beta",
                "Playing",
                false,
                None,
                None,
            ),
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
                None,
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
                None,
            ),
        );

        let snapshot = build_snapshot(&cache);
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].playback_status, "Playing");
    }

    #[test]
    fn build_snapshot_dedupes_browser_bridge_with_same_source_pid() {
        let mut cache = HashMap::new();
        let mut brave = make_info(
            "org.mpris.MediaPlayer2.brave.instance",
            "Brave Origin",
            "Playing",
            false,
            Some("brave"),
            Some(103_380),
        );
        brave.title = "Rumble".to_string();
        brave.artist.clear();
        let mut plasma_bridge = make_info(
            "org.mpris.MediaPlayer2.plasma-browser-integration",
            "Chromium",
            "Playing",
            true,
            Some("chromium"),
            Some(103_380),
        );
        plasma_bridge.title =
            "LA Mayor Karen Bass suffers POLITICAL EXPLOSION as DEMS CRY RACISM".to_string();
        plasma_bridge.artist = "DeVory Darkins".to_string();
        cache.insert(brave.bus_name.clone(), brave);
        cache.insert(plasma_bridge.bus_name.clone(), plasma_bridge);

        let snapshot = build_snapshot(&cache);
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].identity, "Chromium");
    }

    #[test]
    fn build_snapshot_keeps_distinct_browser_tracks() {
        let mut cache = HashMap::new();
        let mut brave = make_info(
            "org.mpris.MediaPlayer2.brave.instance",
            "Brave",
            "Playing",
            false,
            Some("brave"),
            Some(11),
        );
        brave.title = "first track".to_string();
        let mut chromium = make_info(
            "org.mpris.MediaPlayer2.chromium.instance",
            "Chromium",
            "Playing",
            false,
            Some("chromium"),
            Some(22),
        );
        chromium.title = "second track".to_string();
        cache.insert(brave.bus_name.clone(), brave);
        cache.insert(chromium.bus_name.clone(), chromium);

        let snapshot = build_snapshot(&cache);
        assert_eq!(snapshot.len(), 2);
    }

    #[test]
    fn normalize_token_compacts_and_lowercases() {
        let token = normalize_token("  Foo--Bar\tBaz  ");
        // Hyphens are treated as punctuation; only whitespace yields word boundaries
        assert_eq!(token, "foobar baz");
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
                make_info(
                    "org.mpris.MediaPlayer2.a",
                    "Alpha",
                    "Playing",
                    true,
                    None,
                    None,
                ),
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
