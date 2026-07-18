use unixnotis_core::{MediaConfig, MediaRemoteArtPolicy};

use crate::media::MediaCommand;

use super::super::normalize_media_config;
use super::super::r#loop::drain_stale_media_commands;

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

#[test]
fn reconnect_drops_player_commands_from_the_previous_bus_generation() {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(4);
    sender
        .try_send(MediaCommand::Next {
            bus_name: "org.mpris.MediaPlayer2.old".to_string(),
        })
        .expect("queue stale player command");
    sender
        .try_send(MediaCommand::Refresh)
        .expect("queue redundant refresh");

    drain_stale_media_commands(&mut receiver);

    assert!(matches!(
        receiver.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));
}
