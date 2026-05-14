use unixnotis_core::{MediaConfig, MediaRemoteArtPolicy};

use super::normalize_media_config;

#[test]
fn normalize_media_config_lowercases_all_matching_lists() {
    let config = MediaConfig {
        allowlist: vec!["Spotify".to_string(), "VLC".to_string()],
        denylist: vec!["PlayerCtlD".to_string()],
        browser_tokens: vec!["Firefox".to_string(), "Brave".to_string()],
        remote_art_policy: MediaRemoteArtPolicy::BrowsersToo,
        ..MediaConfig::default()
    };

    let normalized = normalize_media_config(config);

    // Player matching is case-insensitive after startup normalization
    assert_eq!(normalized.allowlist, vec!["spotify", "vlc"]);
    assert_eq!(normalized.denylist, vec!["playerctld"]);
    assert_eq!(normalized.browser_tokens, vec!["firefox", "brave"]);
    assert_eq!(
        normalized.remote_art_policy,
        MediaRemoteArtPolicy::BrowsersToo
    );
}
