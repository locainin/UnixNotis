use std::collections::HashMap;
use std::path::PathBuf;

use tokio::runtime::Builder;

use super::super::snapshot::{build_snapshot, normalize_token, send_snapshot_if_changed};
use super::support::receive_ui_event;
use crate::control::UiEvent;
use crate::media::{MediaArtSource, MediaInfo};

const SOURCE_BROWSER_PID: u32 = 42_424;

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
        browser_family: browser_family.map(std::string::ToString::to_string),
        owner_pid,
        source_pid_hint: None,
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
fn build_snapshot_keeps_paused_players_before_inactive_sessions() {
    let mut cache = HashMap::new();
    cache.insert(
        "org.mpris.MediaPlayer2.stopped".to_string(),
        make_info(
            "org.mpris.MediaPlayer2.stopped",
            "Stopped",
            "Stopped",
            false,
            None,
            None,
        ),
    );
    cache.insert(
        "org.mpris.MediaPlayer2.paused".to_string(),
        make_info(
            "org.mpris.MediaPlayer2.paused",
            "Paused",
            "Paused",
            false,
            None,
            None,
        ),
    );

    let snapshot = build_snapshot(&cache);
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].identity, "Paused");
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
        Some(SOURCE_BROWSER_PID),
    );
    brave.title = "Rumble".to_string();
    brave.artist.clear();
    let mut plasma_bridge = make_info(
        "org.mpris.MediaPlayer2.plasma-browser-integration",
        "Chromium",
        "Playing",
        true,
        Some("chromium"),
        Some(22),
    );
    plasma_bridge.source_pid_hint = Some(SOURCE_BROWSER_PID);
    plasma_bridge.title = "A Long Tutorial With Several Chapters".to_string();
    plasma_bridge.artist = "Example Artist".to_string();
    cache.insert(brave.bus_name.clone(), brave);
    cache.insert(plasma_bridge.bus_name.clone(), plasma_bridge);

    let snapshot = build_snapshot(&cache);
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].identity, "Chromium");
}

#[test]
fn source_pid_hint_dedupes_when_bridge_family_is_unresolved() {
    let browser_pid = 42_424;

    let direct = make_info(
        "org.mpris.MediaPlayer2.brave.instance",
        "Brave",
        "Playing",
        false,
        Some("brave"),
        Some(browser_pid),
    );

    let mut bridge = make_info(
        "org.mpris.MediaPlayer2.plasma-browser-integration",
        "Plasma Browser Integration",
        "Playing",
        true,
        None,
        Some(2_400),
    );
    bridge.source_pid_hint = Some(browser_pid);
    bridge.title = "Completely different bridge metadata".to_string();
    bridge.artist = "Different artist".to_string();

    let cache = HashMap::from([
        (direct.bus_name.clone(), direct),
        (bridge.bus_name.clone(), bridge),
    ]);

    assert_eq!(build_snapshot(&cache).len(), 1);
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
fn identical_browser_metadata_with_different_processes_remains_separate() {
    let mut first = make_info(
        "org.mpris.MediaPlayer2.first",
        "First",
        "Playing",
        false,
        Some("first"),
        Some(11),
    );
    first.title = "Shared Video".to_string();
    first.artist = "Shared Creator".to_string();

    let mut second = make_info(
        "org.mpris.MediaPlayer2.second",
        "Second",
        "Playing",
        false,
        Some("second"),
        Some(22),
    );
    second.title = first.title.clone();
    second.artist = first.artist.clone();

    let mut cache = HashMap::new();
    cache.insert(first.bus_name.clone(), first);
    cache.insert(second.bus_name.clone(), second);

    let snapshot = build_snapshot(&cache);
    assert_eq!(snapshot.len(), 2);
}

#[test]
fn empty_browser_artist_does_not_create_a_cross_browser_track_key() {
    let mut cache = HashMap::new();
    let mut first = make_info(
        "org.mpris.MediaPlayer2.first",
        "First",
        "Playing",
        false,
        Some("first"),
        Some(11),
    );
    first.title = "Generic stream".to_string();
    first.artist.clear();
    let mut second = make_info(
        "org.mpris.MediaPlayer2.second",
        "Second",
        "Playing",
        false,
        Some("second"),
        Some(22),
    );
    second.title = first.title.clone();
    second.artist.clear();
    cache.insert(first.bus_name.clone(), first);
    cache.insert(second.bus_name.clone(), second);

    assert_eq!(build_snapshot(&cache).len(), 2);
}

#[test]
fn build_snapshot_collapses_same_track_across_browser_families() {
    let mut cache = HashMap::new();
    let mut chromium = make_info(
        "org.mpris.MediaPlayer2.chromium.instance",
        "Chromium",
        "Playing",
        false,
        Some("chromium"),
        None,
    );
    chromium.title = "The Thing 1982 - What does it mean".to_string();
    chromium.artist = "That Scouse Dude".to_string();
    let mut brave = make_info(
        "org.mpris.MediaPlayer2.brave.instance",
        "Brave",
        "Playing",
        true,
        Some("brave"),
        None,
    );
    brave.title = chromium.title.clone();
    brave.artist = chromium.artist.clone();
    cache.insert(chromium.bus_name.clone(), chromium);
    cache.insert(brave.bus_name.clone(), brave);

    let snapshot = build_snapshot(&cache);

    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].identity, "Brave");
    assert!(snapshot[0].art_source.is_some());
}

#[test]
fn distinct_bridge_sources_owned_by_one_helper_remain_separate() {
    let mut cache = HashMap::new();
    let mut first = make_info(
        "org.mpris.MediaPlayer2.plasma-browser-integration.first",
        "First bridge",
        "Playing",
        true,
        Some("chromium"),
        Some(2_400),
    );
    first.source_pid_hint = Some(11_000);

    let mut second = make_info(
        "org.mpris.MediaPlayer2.plasma-browser-integration.second",
        "Second bridge",
        "Playing",
        true,
        Some("chromium"),
        Some(2_400),
    );
    second.source_pid_hint = Some(22_000);

    cache.insert(first.bus_name.clone(), first);
    cache.insert(second.bus_name.clone(), second);

    let snapshot = build_snapshot(&cache);
    assert_eq!(snapshot.len(), 2);
    assert_eq!(
        snapshot
            .iter()
            .map(|info| info.identity.as_str())
            .collect::<Vec<_>>(),
        vec!["First bridge", "Second bridge"]
    );
}

#[test]
fn browser_sources_match_direct_players_without_crossjoining_a_shared_helper() {
    let mut cache = HashMap::new();
    let mut direct_first = make_info(
        "org.mpris.MediaPlayer2.first",
        "First browser",
        "Playing",
        false,
        Some("first"),
        Some(11_000),
    );
    direct_first.title = "First track".to_string();
    direct_first.artist = "First artist".to_string();

    let mut bridge_first = make_info(
        "org.mpris.MediaPlayer2.plasma-browser-integration.first",
        "First bridge",
        "Playing",
        true,
        Some("chromium"),
        Some(2_400),
    );
    bridge_first.source_pid_hint = Some(11_000);
    bridge_first.title = "Different bridge metadata".to_string();
    bridge_first.artist = "Different bridge artist".to_string();

    let mut direct_second = make_info(
        "org.mpris.MediaPlayer2.second",
        "Second browser",
        "Playing",
        false,
        Some("second"),
        Some(22_000),
    );
    direct_second.title = "Second track".to_string();
    direct_second.artist = "Second artist".to_string();

    let mut bridge_second = make_info(
        "org.mpris.MediaPlayer2.plasma-browser-integration.second",
        "Second bridge",
        "Playing",
        true,
        Some("chromium"),
        Some(2_400),
    );
    bridge_second.source_pid_hint = Some(22_000);
    bridge_second.title = "Another bridge metadata".to_string();
    bridge_second.artist = "Another bridge artist".to_string();

    for info in [direct_first, bridge_first, direct_second, bridge_second] {
        cache.insert(info.bus_name.clone(), info);
    }

    let snapshot = build_snapshot(&cache);
    assert_eq!(snapshot.len(), 2);
    assert_eq!(
        snapshot
            .iter()
            .map(|info| info.identity.as_str())
            .collect::<Vec<_>>(),
        vec!["First bridge", "Second bridge"]
    );
}

#[test]
fn browser_track_key_requires_artist_to_avoid_generic_title_collisions() {
    let mut cache = HashMap::new();
    let mut first = make_info(
        "org.mpris.MediaPlayer2.first",
        "First",
        "Playing",
        false,
        Some("first"),
        None,
    );
    first.title = "YouTube".to_string();
    first.artist.clear();

    let mut second = first.clone();
    second.bus_name = "org.mpris.MediaPlayer2.second".to_string();
    second.identity = "Second".to_string();
    second.browser_family = Some("second".to_string());

    cache.insert(first.bus_name.clone(), first);
    cache.insert(second.bus_name.clone(), second);

    assert_eq!(build_snapshot(&cache).len(), 2);
}

#[test]
fn browser_source_pid_bridges_different_metadata() {
    let mut cache = HashMap::new();
    let mut brave = make_info(
        "org.mpris.MediaPlayer2.brave.instance",
        "Brave Origin",
        "Playing",
        false,
        Some("brave"),
        Some(SOURCE_BROWSER_PID),
    );
    brave.title = "A Long Tutorial With Several Chapters - YouTube".to_string();
    brave.artist.clear();

    let mut bridge = make_info(
        "org.mpris.MediaPlayer2.plasma-browser-integration",
        "Chromium",
        "Paused",
        true,
        Some("chromium"),
        Some(22),
    );
    bridge.source_pid_hint = Some(SOURCE_BROWSER_PID);
    bridge.title = "A Long Tutorial With Several Chapters".to_string();
    bridge.artist = "Example Artist".to_string();

    cache.insert(brave.bus_name.clone(), brave);
    cache.insert(bridge.bus_name.clone(), bridge);

    let snapshot = build_snapshot(&cache);

    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].identity, "Brave Origin");
}

#[test]
fn browser_players_with_different_process_pids_remain_separate() {
    let mut cache = HashMap::new();
    let mut first = make_info(
        "org.mpris.MediaPlayer2.first",
        "First",
        "Playing",
        false,
        Some("first"),
        Some(11),
    );
    first.title = "One Two Three Four".to_string();
    first.artist.clear();

    let mut second = first.clone();
    second.bus_name = "org.mpris.MediaPlayer2.second".to_string();
    second.identity = "Second".to_string();
    second.browser_family = Some("second".to_string());
    second.owner_pid = Some(22);

    cache.insert(first.bus_name.clone(), first);
    cache.insert(second.bus_name.clone(), second);

    // Four short words do not carry enough identity to bridge unrelated browser sessions
    assert_eq!(build_snapshot(&cache).len(), 2);
}

#[test]
fn duplicate_components_keep_first_component_order_when_art_selects_later_entry() {
    let mut cache = HashMap::new();
    let mut first = make_info(
        "org.mpris.MediaPlayer2.component-a-first",
        "Alpha",
        "Playing",
        false,
        Some("alpha"),
        None,
    );
    first.title = "Component A".to_string();
    first.artist = "Artist A".to_string();

    let mut middle = make_info(
        "org.mpris.MediaPlayer2.component-b",
        "Beta",
        "Playing",
        false,
        Some("beta"),
        None,
    );
    middle.title = "Component B".to_string();
    middle.artist = "Artist B".to_string();

    let mut later = first.clone();
    later.bus_name = "org.mpris.MediaPlayer2.component-a-later".to_string();
    later.identity = "Zeta".to_string();
    later.browser_family = Some("zeta".to_string());
    later.owner_pid = None;
    later.art_source = Some(MediaArtSource::LocalFile(PathBuf::from("/tmp/art.png")));

    cache.insert(first.bus_name.clone(), first);
    cache.insert(middle.bus_name.clone(), middle);
    cache.insert(later.bus_name.clone(), later);

    let snapshot = build_snapshot(&cache);

    assert_eq!(snapshot.len(), 2);
    assert_eq!(snapshot[0].identity, "Zeta");
    assert_eq!(snapshot[1].identity, "Beta");
}

#[test]
fn duplicate_selection_prefers_artwork_even_when_that_entry_sorts_later() {
    let mut cache = HashMap::new();
    let mut no_art = make_info(
        "org.mpris.MediaPlayer2.no-art",
        "Alpha",
        "Playing",
        false,
        Some("alpha"),
        None,
    );
    no_art.title = "Shared Long Tutorial Title With Context".to_string();
    no_art.artist = "Shared Artist".to_string();

    let mut with_art = no_art.clone();
    with_art.bus_name = "org.mpris.MediaPlayer2.with-art".to_string();
    with_art.identity = "Zeta".to_string();
    with_art.browser_family = Some("zeta".to_string());
    with_art.owner_pid = None;
    with_art.art_source = Some(MediaArtSource::LocalFile(PathBuf::from("/tmp/art.png")));

    cache.insert(no_art.bus_name.clone(), no_art);
    cache.insert(with_art.bus_name.clone(), with_art);

    let snapshot = build_snapshot(&cache);

    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].identity, "Zeta");
    assert!(snapshot[0].art_source.is_some());
}

#[test]
fn equal_score_duplicate_uses_bus_name_as_a_stable_tie_breaker() {
    let mut cache = HashMap::new();
    let mut first = make_info(
        "org.mpris.MediaPlayer2.z-order",
        "Alpha",
        "Playing",
        false,
        Some("alpha"),
        None,
    );
    first.title = "Shared Track With Stable Metadata".to_string();
    first.artist = "Shared Artist".to_string();

    let mut second = first.clone();
    second.bus_name = "org.mpris.MediaPlayer2.a-order".to_string();
    second.identity = "Zeta".to_string();
    second.browser_family = Some("zeta".to_string());
    second.owner_pid = None;

    cache.insert(first.bus_name.clone(), first);
    cache.insert(second.bus_name.clone(), second);

    let snapshot = build_snapshot(&cache);

    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].identity, "Zeta");
}

#[test]
fn equal_bus_name_duplicate_keeps_the_first_equal_score_entry() {
    let mut cache = HashMap::new();
    let mut first = make_info(
        "org.mpris.MediaPlayer2.same",
        "Alpha",
        "Playing",
        false,
        Some("alpha"),
        None,
    );
    first.title = "Shared Track With Stable Metadata".to_string();
    first.artist = "Shared Artist".to_string();

    let mut second = first.clone();
    second.identity = "Zeta".to_string();
    second.browser_family = Some("zeta".to_string());
    second.owner_pid = None;

    cache.insert("entry-a".to_string(), first);
    cache.insert("entry-b".to_string(), second);

    let snapshot = build_snapshot(&cache);

    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].identity, "Alpha");
}

#[test]
fn build_snapshot_keeps_the_first_equal_score_duplicate() {
    let mut cache = HashMap::new();
    let mut first = make_info(
        "org.mpris.MediaPlayer2.first",
        "First",
        "Playing",
        true,
        Some("first"),
        Some(11),
    );
    first.title = "shared track".to_string();
    let mut second = first.clone();
    second.bus_name = "org.mpris.MediaPlayer2.second".to_string();
    second.identity = "Second".to_string();
    second.browser_family = Some("second".to_string());
    cache.insert(first.bus_name.clone(), first);
    cache.insert(second.bus_name.clone(), second);

    let snapshot = build_snapshot(&cache);
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].identity, "First");
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
        match receive_ui_event(&rx).await {
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
        assert!(matches!(receive_ui_event(&rx).await, UiEvent::MediaCleared));

        send_snapshot_if_changed(&tx, &cache, &mut last_snapshot).await;
        assert!(rx.is_empty());
    });
}
