use unixnotis_core::MediaConfig;

use super::super::constants::{MPRIS_APP, MPRIS_PATH, MPRIS_PLAYER, MPRIS_PREFIX};
use super::super::player::{
    build_player_state, fetch_identity, owner_probe_is_stable, resolve_player_owner,
};
use super::support::{MprisFixture, TEST_PLAYER_IDENTITY, TEST_PLAYER_NAME};

#[test]
fn owner_probe_accepts_only_one_stable_unique_owner() {
    assert!(owner_probe_is_stable(":1.40", ":1.40"));
    assert!(!owner_probe_is_stable(":1.40", ":1.41"));
}

#[test]
fn player_proxy_constants_match_the_mpris_contract() {
    assert_eq!(MPRIS_PREFIX, "org.mpris.MediaPlayer2.");
    assert_eq!(MPRIS_PATH, "/org/mpris/MediaPlayer2");
    assert_eq!(MPRIS_PLAYER, "org.mpris.MediaPlayer2.Player");
    assert_eq!(MPRIS_APP, "org.mpris.MediaPlayer2");
}

#[tokio::test]
async fn player_state_uses_live_identity_owner_and_process_details() {
    let fixture = MprisFixture::start().await;

    // Test with default config (empty allowlist, ExactExecutableOnly policy)
    let state = build_player_state(&fixture.client, TEST_PLAYER_NAME, &MediaConfig::default())
        .await
        .expect("probe test MPRIS player")
        .expect("stable test MPRIS owner");

    assert_eq!(state.bus_name, TEST_PLAYER_NAME);
    assert_eq!(state.identity, TEST_PLAYER_IDENTITY);
    assert_eq!(state.owner_pid, Some(std::process::id()));
    assert!(state.remote_art_allowed);
    // With empty allowlist and ExactExecutableOnly policy, local art should be disabled
    assert!(!state.local_art_allowed);
    assert_eq!(
        state.unique_owner.as_deref(),
        fixture.server.unique_name().map(|name| name.as_str())
    );
    #[cfg(target_os = "linux")]
    assert_eq!(
        state.player.destination().as_str(),
        state
            .unique_owner
            .as_deref()
            .expect("captured unique owner")
    );

    let owner = resolve_player_owner(&fixture.client, TEST_PLAYER_NAME)
        .await
        .expect("resolve stable test owner");
    assert_eq!(owner.0, state.unique_owner.expect("captured unique owner"));
    assert_eq!(owner.1, Some(std::process::id()));
    #[cfg(target_os = "linux")]
    assert_eq!(
        owner.2.as_deref(),
        Some(
            std::env::current_exe()
                .expect("resolve current test executable")
                .to_string_lossy()
                .as_ref()
        )
    );
    assert_eq!(
        fetch_identity(&fixture.client, owner.0.as_str()).await,
        Some(TEST_PLAYER_IDENTITY.to_string())
    );
}
