use unixnotis_core::MediaRemoteArtPolicy;

use super::{detect_browser_family, normalize_art_source, remote_art_allowed};
use crate::media::MediaArtSource;

#[test]
fn normalize_art_source_keeps_local_files() {
    let source = normalize_art_source("file:///tmp/track%20art.png", false);
    assert!(matches!(source, Some(MediaArtSource::LocalFile(_))));
}

#[test]
fn normalize_art_source_rejects_insecure_or_local_remote_targets() {
    assert!(normalize_art_source("http://example.com/art.png", true).is_none());
    assert!(normalize_art_source("https://127.0.0.1/art.png", true).is_none());
    assert!(normalize_art_source("https://localhost/art.png", true).is_none());
}

#[test]
fn normalize_art_source_accepts_https_when_remote_is_allowed() {
    let source = normalize_art_source("https://example.com/art.png", true);
    assert!(matches!(source, Some(MediaArtSource::RemoteHttps(_))));
}

#[test]
fn detect_browser_family_matches_bus_or_identity() {
    let tokens = vec!["firefox".to_string()];
    assert_eq!(
        detect_browser_family(
            "Firefox",
            "org.mpris.MediaPlayer2.firefox.instance",
            &tokens
        ),
        Some("firefox".to_string())
    );
}

#[test]
fn remote_art_policy_blocks_browsers_by_default() {
    assert!(!remote_art_allowed(
        Some("firefox"),
        Some("/usr/bin/firefox"),
        MediaRemoteArtPolicy::NativeOnly
    ));
    assert!(remote_art_allowed(
        None,
        Some("/usr/bin/spotify"),
        MediaRemoteArtPolicy::NativeOnly
    ));
    assert!(!remote_art_allowed(
        None,
        None,
        MediaRemoteArtPolicy::BrowsersToo
    ));
}

#[test]
fn detect_browser_family_does_not_match_inner_substrings() {
    let tokens = vec!["zen".to_string(), "edge".to_string()];

    assert_eq!(
        detect_browser_family("Zenity Helper", "org.mpris.MediaPlayer2.zenity", &tokens),
        None
    );
    assert_eq!(
        detect_browser_family(
            "Knowledge Player",
            "org.mpris.MediaPlayer2.knowledge",
            &tokens
        ),
        None
    );
    assert_eq!(
        detect_browser_family(
            "Microsoft Edge",
            "org.mpris.MediaPlayer2.microsoft-edge",
            &tokens
        ),
        Some("edge".to_string())
    );
}
