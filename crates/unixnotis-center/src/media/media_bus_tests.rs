use std::collections::HashMap;

use unixnotis_core::MediaConfig;

use super::{is_allowed_player, is_relevant_media_change};

#[test]
fn is_allowed_player_respects_lists() {
    let mut config = MediaConfig {
        // This matches the default hardening where browser players stay opt-in
        include_browsers: false,
        allowlist: vec!["spotify".to_string()],
        denylist: vec!["playerctld".to_string()],
        ..MediaConfig::default()
    };
    // Lowercasing here mirrors the runtime sanitize path
    config.allowlist = config
        .allowlist
        .into_iter()
        .map(|entry| entry.to_lowercase())
        .collect();
    config.denylist = config
        .denylist
        .into_iter()
        .map(|entry| entry.to_lowercase())
        .collect();
    config.browser_tokens = config
        .browser_tokens
        .into_iter()
        .map(|entry| entry.to_lowercase())
        .collect();

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
fn relevant_media_change_detects_updates() {
    let mut changed = HashMap::new();
    // Metadata changes are enough to rebuild the visible card
    changed.insert("Metadata", zbus::zvariant::Value::from("track"));
    let invalidated: [&str; 0] = [];

    assert!(is_relevant_media_change(&changed, &invalidated));
}

#[test]
fn relevant_media_change_detects_invalidations() {
    let changed: HashMap<&str, zbus::zvariant::Value<'_>> = HashMap::new();
    // Invalidated capability flags still need a refresh pass
    let invalidated = ["CanPlay"];

    assert!(is_relevant_media_change(&changed, &invalidated));
}

#[test]
fn relevant_media_change_ignores_unrelated_updates() {
    let mut changed = HashMap::new();
    // Volume changes do not affect the compact center card
    changed.insert("Volume", zbus::zvariant::Value::from(0.5_f64));
    let invalidated = ["Position"];

    assert!(!is_relevant_media_change(&changed, &invalidated));
}
