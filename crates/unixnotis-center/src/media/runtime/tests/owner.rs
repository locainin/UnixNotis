use super::super::owner::{
    apply_owner_change, owner_is_unchanged, owner_rebuild_outcome,
    replacement_removal_needs_snapshot, OwnerChangeOutcome,
};
use super::super::state::MediaRuntimeState;
use crate::control::UiEvent;
use crate::media::mpris::tests::support::{MprisFixture, TEST_PLAYER_NAME};
use crate::media::mpris::{build_player_state, fetch_media_info};
use unixnotis_core::MediaConfig;

async fn live_runtime_state(fixture: &MprisFixture) -> MediaRuntimeState {
    // Seed every state layer so removal must update players, cache, and publication
    let player = build_player_state(&fixture.client, TEST_PLAYER_NAME, &MediaConfig::default())
        .await
        .expect("probe test MPRIS player")
        .expect("stable test MPRIS owner");
    let info = fetch_media_info(&player)
        .await
        .expect("fetch test MPRIS metadata");
    let mut state = MediaRuntimeState::new();
    state.players.insert(TEST_PLAYER_NAME.to_string(), player);
    state
        .cache
        .insert(TEST_PLAYER_NAME.to_string(), info.clone());
    state.last_snapshot.push(info);
    state
}

#[test]
fn owner_replacement_rebuilds_state_but_duplicate_signal_does_not() {
    assert!(owner_is_unchanged(Some(":1.42"), Some(":1.42")));
    assert!(!owner_is_unchanged(Some(":1.42"), Some(":1.43")));
    assert!(!owner_is_unchanged(None, Some(":1.43")));
}

#[test]
fn unstable_owner_probe_requests_retry_after_removed_cache_is_published() {
    let outcome = owner_rebuild_outcome(false);

    assert_eq!(outcome, OwnerChangeOutcome::RetryNeeded);
    assert!(replacement_removal_needs_snapshot(true, outcome));
}

#[test]
fn stable_owner_rebuild_does_not_publish_an_empty_replacement_snapshot() {
    let outcome = owner_rebuild_outcome(true);

    assert_eq!(outcome, OwnerChangeOutcome::Applied);
    assert!(!replacement_removal_needs_snapshot(true, outcome));
}

#[tokio::test]
async fn unrelated_owner_change_is_ignored() {
    let fixture = MprisFixture::start().await;
    let (signal_tx, _signal_rx) = tokio::sync::mpsc::channel(4);
    let (event_tx, _event_rx) = async_channel::bounded(4);
    let mut state = MediaRuntimeState::new();

    let outcome = apply_owner_change(
        "org.example.Service",
        None,
        &fixture.client,
        &MediaConfig::default(),
        &signal_tx,
        &mut state,
        &event_tx,
    )
    .await
    .expect("ignore unrelated owner change");

    assert_eq!(outcome, OwnerChangeOutcome::Applied);
}

#[tokio::test]
async fn denied_player_owner_change_removes_existing_state_and_snapshot() {
    let fixture = MprisFixture::start().await;
    let (signal_tx, _signal_rx) = tokio::sync::mpsc::channel(4);
    let (event_tx, event_rx) = async_channel::bounded(4);
    let mut state = live_runtime_state(&fixture).await;
    let config = MediaConfig {
        // The token matches only the fixture player and leaves default policy intact
        denylist: vec!["unixnotis_test".to_string()],
        ..MediaConfig::default()
    };

    let outcome = apply_owner_change(
        TEST_PLAYER_NAME,
        fixture.server.unique_name().map(|name| name.as_str()),
        &fixture.client,
        &config,
        &signal_tx,
        &mut state,
        &event_tx,
    )
    .await
    .expect("remove denied player");

    assert_eq!(outcome, OwnerChangeOutcome::Removed);
    assert!(state.players.is_empty());
    assert!(state.cache.is_empty());
    assert!(matches!(event_rx.recv().await, Ok(UiEvent::MediaCleared)));
}

#[tokio::test]
async fn empty_owner_change_removes_existing_player() {
    let fixture = MprisFixture::start().await;
    let (signal_tx, _signal_rx) = tokio::sync::mpsc::channel(4);
    let (event_tx, event_rx) = async_channel::bounded(4);
    let mut state = live_runtime_state(&fixture).await;

    // An explicit empty owner has the same meaning as a missing owner on D-Bus
    let outcome = apply_owner_change(
        TEST_PLAYER_NAME,
        Some(""),
        &fixture.client,
        &MediaConfig::default(),
        &signal_tx,
        &mut state,
        &event_tx,
    )
    .await
    .expect("remove ownerless player");

    assert_eq!(outcome, OwnerChangeOutcome::Removed);
    assert!(state.players.is_empty());
    assert!(state.cache.is_empty());
    assert!(matches!(event_rx.recv().await, Ok(UiEvent::MediaCleared)));
}

#[tokio::test]
async fn duplicate_owner_change_preserves_existing_player() {
    let fixture = MprisFixture::start().await;
    let (signal_tx, _signal_rx) = tokio::sync::mpsc::channel(4);
    let (event_tx, event_rx) = async_channel::bounded(4);
    let mut state = live_runtime_state(&fixture).await;
    // The captured unique name represents the same process generation
    let owner = state.players[TEST_PLAYER_NAME]
        .unique_owner
        .clone()
        .expect("captured unique owner");

    let outcome = apply_owner_change(
        TEST_PLAYER_NAME,
        Some(owner.as_str()),
        &fixture.client,
        &MediaConfig::default(),
        &signal_tx,
        &mut state,
        &event_tx,
    )
    .await
    .expect("ignore duplicate owner change");

    assert_eq!(outcome, OwnerChangeOutcome::Applied);
    assert!(state.players.contains_key(TEST_PLAYER_NAME));
    assert!(event_rx.is_empty());
}
