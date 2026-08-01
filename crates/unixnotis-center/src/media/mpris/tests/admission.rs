use unixnotis_core::{MediaConfig, MediaLocalArtPolicy, MediaRemoteArtPolicy};

use super::super::admission::{detect_browser_family, local_art_allowed, remote_art_allowed};
use super::super::is_allowed_player;

#[test]
fn player_admission_respects_allow_deny_and_browser_lists() {
    let config = MediaConfig {
        include_browsers: false,
        allowlist: vec!["spotify".to_string()],
        denylist: vec!["playerctld".to_string()],
        ..MediaConfig::default()
    };

    assert!(is_allowed_player("org.mpris.MediaPlayer2.spotify", &config));
    assert!(!is_allowed_player(
        "org.mpris.MediaPlayer2.playerctld",
        &config
    ));
    assert!(!is_allowed_player(
        "org.mpris.MediaPlayer2.firefox",
        &config
    ));
}

#[test]
fn browser_token_matching_avoids_inner_substring_hits() {
    let config = MediaConfig {
        include_browsers: false,
        allowlist: Vec::new(),
        denylist: Vec::new(),
        browser_tokens: vec!["zen".to_string(), "edge".to_string()],
        ..MediaConfig::default()
    };

    assert!(is_allowed_player("org.mpris.MediaPlayer2.zenity", &config));
    assert!(is_allowed_player(
        "org.mpris.MediaPlayer2.knowledge",
        &config
    ));
    assert!(!is_allowed_player(
        "org.mpris.MediaPlayer2.microsoft-edge",
        &config
    ));
}

#[test]
fn browser_family_matches_segments_in_bus_names_or_identities() {
    let tokens = vec!["firefox".to_string(), "edge".to_string()];

    assert_eq!(
        detect_browser_family(
            "Firefox",
            "org.mpris.MediaPlayer2.firefox.instance",
            &tokens
        ),
        Some("firefox".to_string())
    );
    assert_eq!(
        detect_browser_family(
            "Microsoft Edge",
            "org.mpris.MediaPlayer2.microsoft-edge",
            &tokens
        ),
        Some("edge".to_string())
    );
    assert_eq!(
        detect_browser_family(
            "Knowledge Player",
            "org.mpris.MediaPlayer2.knowledge",
            &tokens
        ),
        None
    );
}

#[test]
fn browser_identity_uses_the_mpris_suffix_when_no_token_matches() {
    let tokens = vec!["firefox".to_string()];

    assert_eq!(
        detect_browser_family(
            "Generic Browser",
            "org.mpris.MediaPlayer2.chromium.instance42",
            &tokens,
        ),
        Some("chromium".to_string())
    );
}

#[test]
fn remote_art_admission_keeps_browsers_opt_in_and_requires_an_owner() {
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
fn local_art_admission_rejects_browsers_and_requires_an_owner() {
    // Browser with owner executable should be rejected
    assert!(!local_art_allowed(
        Some("firefox"),
        Some("/usr/bin/firefox"),
        false,
        MediaLocalArtPolicy::ExactExecutableOnly,
    ));

    // Non-browser without allowlist match should be rejected
    assert!(!local_art_allowed(
        None,
        Some("/usr/bin/spotify"),
        false,
        MediaLocalArtPolicy::ExactExecutableOnly,
    ));

    // Non-browser without owner executable should be rejected
    assert!(!local_art_allowed(
        None,
        None,
        false,
        MediaLocalArtPolicy::ExactExecutableOnly,
    ));
}

#[test]
fn local_art_admission_requires_verified_executable_evidence() {
    // A verified descriptor comparison is the only exact-policy admission proof
    assert!(local_art_allowed(
        None,
        Some("/usr/bin/player"),
        true,
        MediaLocalArtPolicy::ExactExecutableOnly,
    ));

    // A path hint without descriptor proof remains denied
    assert!(!local_art_allowed(
        None,
        Some("/usr/bin/player"),
        false,
        MediaLocalArtPolicy::ExactExecutableOnly,
    ));
}
