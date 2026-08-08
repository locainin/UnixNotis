use std::collections::HashSet;

use unixnotis_core::MediaConfig;

use super::super::selection::{is_discoverable_player, select_player_names};

#[test]
fn discovery_requires_an_mpris_name_that_passes_admission() {
    let config = MediaConfig {
        denylist: vec!["blocked".to_string()],
        ..MediaConfig::default()
    };

    assert!(is_discoverable_player(
        "org.mpris.MediaPlayer2.allowed",
        &config
    ));
    assert!(!is_discoverable_player("org.example.allowed", &config));
    assert!(!is_discoverable_player(
        "org.mpris.MediaPlayer2.blocked",
        &config
    ));
}

#[test]
fn discovery_orders_all_names_before_owner_capacity_is_applied() {
    let names = (0..48)
        .map(|index| format!("org.mpris.MediaPlayer2.player-{index:03}"))
        .collect::<HashSet<_>>();
    let mut cursor = 0;
    let selected = select_player_names(names, &HashSet::new(), &mut cursor);

    assert_eq!(selected.len(), 48);
    assert_eq!(
        selected.first().map(String::as_str),
        Some("org.mpris.MediaPlayer2.player-000")
    );
    assert_eq!(
        selected.last().map(String::as_str),
        Some("org.mpris.MediaPlayer2.player-047")
    );
}

#[test]
fn discovery_keeps_all_admitted_names_for_owner_resolution() {
    let names = (0..32)
        .map(|index| format!("org.mpris.MediaPlayer2.player-{index:03}"))
        .collect::<HashSet<_>>();

    let mut cursor = 0;
    assert_eq!(
        select_player_names(names, &HashSet::new(), &mut cursor).len(),
        32
    );
}

#[test]
fn discovery_caps_candidate_work_and_rotates_untracked_names() {
    let names = (0..256)
        .map(|index| format!("org.mpris.MediaPlayer2.player-{index:03}"))
        .collect::<HashSet<_>>();
    let mut cursor = 0;
    let first = select_player_names(names.clone(), &HashSet::new(), &mut cursor);
    let second = select_player_names(names, &HashSet::new(), &mut cursor);

    assert_eq!(first.len(), 128);
    assert_eq!(second.len(), 128);
    assert!(first.iter().all(|name| !second.contains(name)));
}

#[test]
fn discovery_rotation_wraps_from_a_nonzero_cursor() {
    let names = (0..256)
        .map(|index| format!("org.mpris.MediaPlayer2.player-{index:03}"))
        .collect::<HashSet<_>>();
    let mut cursor = 130;

    let selected = select_player_names(names, &HashSet::new(), &mut cursor);

    assert_eq!(
        selected.first().map(String::as_str),
        Some("org.mpris.MediaPlayer2.player-130")
    );
    assert_eq!(cursor, 2);
}

#[test]
fn discovery_always_preserves_tracked_names_before_rotation() {
    let names = (0..256)
        .map(|index| format!("org.mpris.MediaPlayer2.player-{index:03}"))
        .collect::<HashSet<_>>();
    let tracked = HashSet::from([
        "org.mpris.MediaPlayer2.player-255".to_string(),
        "org.mpris.MediaPlayer2.player-254".to_string(),
    ]);
    let mut cursor = 0;
    let selected = select_player_names(names, &tracked, &mut cursor);

    assert!(selected
        .iter()
        .any(|name| name == "org.mpris.MediaPlayer2.player-254"));
    assert!(selected
        .iter()
        .any(|name| name == "org.mpris.MediaPlayer2.player-255"));
    assert_eq!(selected.len(), 128);
}

#[test]
fn discovery_selection_handles_empty_and_full_tracked_pages() {
    let mut cursor = 0;
    assert!(select_player_names(HashSet::new(), &HashSet::new(), &mut cursor).is_empty());

    let names = (0..256)
        .map(|index| format!("org.mpris.MediaPlayer2.player-{index:03}"))
        .collect::<HashSet<_>>();
    let tracked = names.iter().take(128).cloned().collect::<HashSet<_>>();
    let selected = select_player_names(names, &tracked, &mut cursor);

    assert_eq!(selected.len(), 128);
    assert!(selected.iter().all(|name| tracked.contains(name)));
}
