use std::time::{Duration, Instant};

use unixnotis_core::MediaConfig;

use super::super::constants::{MPRIS_APP, MPRIS_PATH, MPRIS_PLAYER, MPRIS_PREFIX};
use super::super::player::{
    build_player_state, build_player_state_for_owner, fetch_identity, owner_probe_is_stable,
    quarantine_active, read_owner_executable_path, resolve_player_owner, PlayerTimeoutState,
};
use super::support::{MprisFixture, TEST_PLAYER_IDENTITY, TEST_PLAYER_NAME};

#[test]
fn owner_probe_accepts_only_one_stable_unique_owner() {
    assert!(owner_probe_is_stable(":1.40", ":1.40"));
    assert!(!owner_probe_is_stable(":1.40", ":1.41"));
}

#[test]
fn quarantine_deadline_is_exclusive() {
    let now = Instant::now();
    assert!(quarantine_active(now, now + Duration::from_millis(1)));
    assert!(!quarantine_active(now, now));
}

#[cfg(target_os = "linux")]
#[test]
fn owner_probe_keeps_metadata_when_process_fd_is_unavailable() {
    let path = read_owner_executable_path(std::process::id(), None)
        .expect("PID fallback should resolve the current executable");

    assert!(path.is_absolute());
}

#[test]
fn player_proxy_constants_match_the_mpris_contract() {
    assert_eq!(MPRIS_PREFIX, "org.mpris.MediaPlayer2.");
    assert_eq!(MPRIS_PATH, "/org/mpris/MediaPlayer2");
    assert_eq!(MPRIS_PLAYER, "org.mpris.MediaPlayer2.Player");
    assert_eq!(MPRIS_APP, "org.mpris.MediaPlayer2");
}

#[test]
fn player_timeout_state_quarantines_after_repeated_failures() {
    let state = PlayerTimeoutState::new();

    assert!(!state.is_quarantined());
    state.record_timeout();
    state.record_timeout();
    assert!(!state.is_quarantined());
    state.record_timeout();
    assert!(state.is_quarantined());
}

#[test]
fn player_timeout_state_clear_releases_a_quarantine() {
    let state = PlayerTimeoutState::new();
    for _ in 0..3 {
        state.record_timeout();
    }

    assert!(state.is_quarantined());
    state.clear_timeout();
    assert!(!state.is_quarantined());
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
    assert_eq!(
        owner.unique_owner.as_str(),
        state.unique_owner.expect("captured unique owner")
    );
    assert_eq!(owner.pid, std::process::id());
    #[cfg(target_os = "linux")]
    assert_eq!(
        owner.executable.as_deref(),
        Some(
            std::env::current_exe()
                .expect("resolve current test executable")
                .to_string_lossy()
                .as_ref()
        )
    );
    assert_eq!(
        fetch_identity(&fixture.client, owner.unique_owner.as_str()).await,
        Some(TEST_PLAYER_IDENTITY.to_string())
    );
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn player_state_without_process_fd_keeps_remote_metadata_and_disables_local_art() {
    let fixture = MprisFixture::start().await;
    let owner = resolve_player_owner(&fixture.client, TEST_PLAYER_NAME)
        .await
        .expect("resolve stable test owner");
    let owner_pid = owner.pid;
    let mut owner_without_process_fd = owner;
    owner_without_process_fd.process_fd = None;

    let state = build_player_state_for_owner(
        &fixture.client,
        TEST_PLAYER_NAME,
        &MediaConfig::default(),
        owner_without_process_fd,
    )
    .await
    .expect("build player state without ProcessFD");

    assert_eq!(state.owner_pid, Some(owner_pid));
    assert!(state.remote_art_allowed);
    assert!(!state.local_art_allowed);
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn exact_local_art_policy_uses_the_connection_process_fd() {
    let fixture = MprisFixture::start().await;
    let current_executable = std::env::current_exe().expect("resolve current test executable");
    let config = MediaConfig {
        local_art_executable_allowlist: vec![current_executable.display().to_string()],
        ..MediaConfig::default()
    };

    let state = build_player_state(&fixture.client, TEST_PLAYER_NAME, &config)
        .await
        .expect("probe test MPRIS player")
        .expect("stable test MPRIS owner");

    assert!(state.local_art_allowed);
}
