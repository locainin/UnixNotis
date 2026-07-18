use std::collections::BTreeMap;

use crate::media::MediaInfo;
use unixnotis_core::{MediaPositionFormat, MediaTitleFallback};

use super::{
    art_slot_visible, artist_text_for, position_text_for, source_text_for, title_text_for,
    MediaDisplayConfig,
};

fn media_info(identity: &str, title: &str, artist: &str) -> MediaInfo {
    MediaInfo {
        bus_name: format!("org.mpris.MediaPlayer2.{identity}"),
        identity: identity.to_string(),
        browser_family: None,
        owner_pid: None,
        title: title.to_string(),
        artist: artist.to_string(),
        playback_status: "Paused".to_string(),
        art_source: None,
        can_play: true,
        can_pause: true,
        can_next: true,
        can_prev: true,
    }
}

fn display() -> MediaDisplayConfig {
    MediaDisplayConfig {
        show_source: true,
        show_source_when_single_player: true,
        show_position: true,
        show_position_when_single_player: false,
        show_title: true,
        show_artist: true,
        collapse_missing_artist: false,
        collapse_missing_art: false,
        title_fallback: MediaTitleFallback::Identity,
        position_format: MediaPositionFormat::Fraction,
        source_aliases: BTreeMap::new(),
    }
}

#[test]
fn missing_metadata_can_collapse_without_hiding_real_values() {
    let mut display = display();
    display.collapse_missing_artist = true;
    display.collapse_missing_art = true;

    assert_eq!(artist_text_for("", &display), None);
    assert_eq!(artist_text_for("Boards", &display), Some("Boards"));
    assert!(!art_slot_visible(false, &display));
    assert!(art_slot_visible(true, &display));
}

#[test]
fn stable_metadata_lanes_reserve_space_by_default() {
    let display = display();

    assert_eq!(artist_text_for("", &display), Some(" "));
    assert!(art_slot_visible(false, &display));
}

#[test]
fn source_alias_prefers_longest_match() {
    let mut display = display();
    display
        .source_aliases
        .insert("spot".to_string(), "Short".to_string());
    display
        .source_aliases
        .insert("spotify".to_string(), "Spotify Player".to_string());
    let info = media_info("Spotify", "", "");

    assert_eq!(
        source_text_for(&info, 2, &display).as_deref(),
        Some("Spotify Player")
    );
}

#[test]
fn title_fallback_can_use_artist() {
    let mut display = display();
    display.title_fallback = MediaTitleFallback::Artist;
    let info = media_info("Spotify", "", "Boards");

    assert_eq!(title_text_for(&info, &display).as_deref(), Some("Boards"));
}

#[test]
fn title_fallback_can_stay_blank() {
    let mut display = display();
    display.title_fallback = MediaTitleFallback::Empty;
    let info = media_info("Spotify", "", "Boards");

    assert_eq!(title_text_for(&info, &display), None);
}

#[test]
fn source_can_hide_for_single_player() {
    let mut display = display();
    display.show_source_when_single_player = false;
    let info = media_info("Spotify", "Track", "");

    assert_eq!(source_text_for(&info, 1, &display), None);
}

#[test]
fn position_can_render_current_only() {
    let mut display = display();
    display.position_format = MediaPositionFormat::Current;

    assert_eq!(position_text_for(2, 4, &display).as_deref(), Some("2"));
}

#[test]
fn blank_identity_falls_back_to_bus_name_tail() {
    let info = MediaInfo {
        bus_name: "org.mpris.MediaPlayer2.chromium.instance123".to_string(),
        identity: String::new(),
        browser_family: None,
        owner_pid: None,
        title: "Track".to_string(),
        artist: String::new(),
        playback_status: "Paused".to_string(),
        art_source: None,
        can_play: true,
        can_pause: true,
        can_next: true,
        can_prev: true,
    };

    assert_eq!(
        source_text_for(&info, 2, &display()).as_deref(),
        Some("instance123")
    );
}

#[test]
fn empty_alias_map_keeps_default_source_label_behavior() {
    let info = media_info("Spotify", "Track", "Artist");

    assert_eq!(
        source_text_for(&info, 2, &display()).as_deref(),
        Some("Spotify")
    );
}
