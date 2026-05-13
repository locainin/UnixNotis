use crate::media::MediaInfo;

use super::{MediaSelection, MediaSelectionSnapshot};

fn media_info(bus_name: &str, title: &str) -> MediaInfo {
    MediaInfo {
        bus_name: bus_name.to_string(),
        identity: bus_name.to_string(),
        browser_family: None,
        owner_pid: None,
        title: title.to_string(),
        artist: String::new(),
        playback_status: "Paused".to_string(),
        art_source: None,
        can_play: true,
        can_pause: true,
        can_next: true,
        can_prev: true,
    }
}

#[test]
fn snapshot_restore_keeps_current_player_when_bus_still_exists() {
    let mut selection = MediaSelection::default();
    selection.set_players(vec![
        media_info("org.mpris.MediaPlayer2.alpha", "Alpha"),
        media_info("org.mpris.MediaPlayer2.beta", "Beta"),
    ]);
    selection.next();

    let snapshot = selection.snapshot();

    let mut restored = MediaSelection::default();
    restored.restore_snapshot(&snapshot);

    assert_eq!(restored.position(), (2, 2));
    assert_eq!(
        restored.current().map(|info| info.bus_name.as_str()),
        Some("org.mpris.MediaPlayer2.beta")
    );
}

#[test]
fn snapshot_restore_falls_back_to_first_player_when_bus_is_gone() {
    let snapshot = MediaSelectionSnapshot {
        players: vec![
            media_info("org.mpris.MediaPlayer2.alpha", "Alpha"),
            media_info("org.mpris.MediaPlayer2.beta", "Beta"),
        ],
        current_bus: Some("org.mpris.MediaPlayer2.missing".to_string()),
    };

    let mut restored = MediaSelection::default();
    restored.restore_snapshot(&snapshot);

    assert_eq!(restored.position(), (1, 2));
    assert_eq!(
        restored.current().map(|info| info.bus_name.as_str()),
        Some("org.mpris.MediaPlayer2.alpha")
    );
}
