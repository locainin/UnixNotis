use super::super::cache::*;
use crate::media::MediaInfo;

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
        browser_family: browser_family.map(std::string::ToString::to_string),
        owner_pid: None,
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
fn merge_media_info_keeps_last_good_entry_when_fetch_fails() {
    let existing = make_info("org.mpris.MediaPlayer2.a", "Alpha", "Playing", true, None);
    let merged =
        merge_media_info(Some(&existing), None, MediaCacheMergeMode::Stable).expect("merged info");

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
fn transition_merge_accepts_new_text_and_art_as_soon_as_they_arrive() {
    let mut existing = make_info("org.mpris.MediaPlayer2.a", "Alpha", "Playing", true, None);
    existing.title = "Old title".to_string();
    existing.art_source = Some(crate::media::MediaArtSource::LocalFile(
        std::path::PathBuf::from("/tmp/old-art.png"),
    ));
    let mut fetched = make_info("org.mpris.MediaPlayer2.a", "Alpha", "Playing", true, None);
    fetched.title = "New title".to_string();
    fetched.art_source = Some(crate::media::MediaArtSource::LocalFile(
        std::path::PathBuf::from("/tmp/new-art.png"),
    ));

    let merged = merge_media_info(
        Some(&existing),
        Some(fetched),
        MediaCacheMergeMode::Transitioning,
    )
    .expect("transition snapshot");

    assert_eq!(merged.title, "New title");
    assert_eq!(
        merged.art_source,
        Some(crate::media::MediaArtSource::LocalFile(
            std::path::PathBuf::from("/tmp/new-art.png")
        ))
    );
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
fn fallback_after_skip_replaces_every_stale_metadata_field() {
    let mut old_track = make_info("org.mpris.MediaPlayer2.a", "Alpha", "Playing", true, None);
    old_track.title = "Old title".to_string();
    old_track.artist = "Old artist".to_string();

    let mut partial = make_info("org.mpris.MediaPlayer2.a", "Alpha", "Playing", false, None);
    partial.title.clear();
    partial.artist.clear();
    let transitioning = merge_media_info(
        Some(&old_track),
        Some(partial),
        MediaCacheMergeMode::Transitioning,
    )
    .expect("transition snapshot");
    assert_eq!(transitioning.title, "Old title");

    let mut new_track = make_info("org.mpris.MediaPlayer2.a", "Alpha", "Playing", false, None);
    new_track.title = "New title".to_string();
    new_track.artist = "New artist".to_string();
    let settled = merge_media_info(
        Some(&transitioning),
        Some(new_track),
        MediaCacheMergeMode::Stable,
    )
    .expect("settled snapshot");

    assert_eq!(settled.title, "New title");
    assert_eq!(settled.artist, "New artist");
    assert!(settled.art_source.is_none());
}

#[test]
fn fallback_after_skip_clears_old_metadata_when_player_stays_blank() {
    let mut old_track = make_info("org.mpris.MediaPlayer2.a", "Alpha", "Playing", true, None);
    old_track.title = "Old title".to_string();
    old_track.artist = "Old artist".to_string();

    let mut blank = make_info("org.mpris.MediaPlayer2.a", "Alpha", "Playing", false, None);
    blank.title.clear();
    blank.artist.clear();
    let settled = merge_media_info(Some(&old_track), Some(blank), MediaCacheMergeMode::Stable)
        .expect("settled blank snapshot");

    assert!(settled.title.is_empty());
    assert!(settled.artist.is_empty());
    assert!(settled.art_source.is_none());
}
