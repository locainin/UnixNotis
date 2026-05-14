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
