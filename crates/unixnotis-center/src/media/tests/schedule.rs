use std::collections::HashMap;

use crate::media::MediaInfo;

use super::needs_metadata_fallback;

fn make_info(status: &str) -> MediaInfo {
    MediaInfo {
        bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
        identity: "Spotify".to_string(),
        browser_family: None,
        owner_pid: None,
        title: "track".to_string(),
        artist: "artist".to_string(),
        playback_status: status.to_string(),
        art_source: None,
        can_play: true,
        can_pause: true,
        can_next: true,
        can_prev: true,
    }
}

#[test]
fn metadata_fallback_stays_on_while_playing() {
    let mut cache = HashMap::new();
    cache.insert(
        "org.mpris.MediaPlayer2.spotify".to_string(),
        make_info("Playing"),
    );

    assert!(needs_metadata_fallback(
        &cache,
        "org.mpris.MediaPlayer2.spotify"
    ));
}

#[test]
fn metadata_fallback_stops_when_not_playing() {
    let mut cache = HashMap::new();
    cache.insert(
        "org.mpris.MediaPlayer2.spotify".to_string(),
        make_info("Paused"),
    );

    assert!(!needs_metadata_fallback(
        &cache,
        "org.mpris.MediaPlayer2.spotify"
    ));
}
