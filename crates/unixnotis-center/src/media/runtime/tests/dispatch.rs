use super::super::cache::MediaCacheMergeMode;
use super::super::dispatch::{
    handle_runtime_command, handle_runtime_signal, merge_mode_for_signal,
    should_publish_immediate_command_snapshot, should_schedule_metadata_fallback,
};
use super::super::state::MediaRuntimeState;
use super::super::{MediaRefreshOrigin, MediaSignal};
use crate::control::UiEvent;
use crate::media::mpris::build_player_state;
use crate::media::mpris::tests::support::{MprisFixture, TEST_PLAYER_NAME};
use crate::media::{MediaCommand, MediaInfo};
use unixnotis_core::MediaConfig;

#[test]
fn property_signal_preserves_player_and_refresh_origin() {
    let signal = MediaSignal::PropertiesChanged {
        bus_name: "org.mpris.MediaPlayer2.test".to_string(),
        origin: MediaRefreshOrigin::Fallback,
    };

    let MediaSignal::PropertiesChanged { bus_name, origin } = signal;
    assert_eq!(bus_name, "org.mpris.MediaPlayer2.test");
    assert_eq!(origin, MediaRefreshOrigin::Fallback);
}

#[test]
fn native_and_synthetic_refresh_origins_remain_distinct() {
    assert_ne!(MediaRefreshOrigin::Bus, MediaRefreshOrigin::Fallback);
}

#[test]
fn fallback_signals_do_not_rearm_but_bus_signals_allow_one_sweep() {
    assert!(!should_schedule_metadata_fallback(
        MediaRefreshOrigin::Fallback
    ));
    assert!(should_schedule_metadata_fallback(MediaRefreshOrigin::Bus));
}

#[test]
fn only_play_pause_commands_publish_an_immediate_snapshot() {
    assert!(should_publish_immediate_command_snapshot(
        &MediaCommand::PlayPause {
            bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
        }
    ));
    for command in [
        MediaCommand::Next {
            bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
        },
        MediaCommand::Previous {
            bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
        },
    ] {
        assert!(!should_publish_immediate_command_snapshot(&command));
    }
}

#[test]
fn bus_updates_transition_while_fallbacks_commit_final_state() {
    assert_eq!(
        merge_mode_for_signal(MediaRefreshOrigin::Bus),
        MediaCacheMergeMode::Transitioning
    );
    assert_eq!(
        merge_mode_for_signal(MediaRefreshOrigin::Fallback),
        MediaCacheMergeMode::Stable
    );
}

#[tokio::test]
async fn runtime_command_dispatches_and_schedules_a_targeted_refresh() {
    let fixture = MprisFixture::start().await;
    let player = build_player_state(&fixture.client, TEST_PLAYER_NAME, &MediaConfig::default())
        .await
        .expect("probe test MPRIS player")
        .expect("stable test MPRIS owner");
    let mut state = MediaRuntimeState::new();
    state.players.insert(TEST_PLAYER_NAME.to_string(), player);
    let (signal_tx, _signal_rx) = tokio::sync::mpsc::channel(4);
    let (event_tx, _event_rx) = async_channel::bounded(4);

    // Next should cross the bus and create one bounded follow-up plan
    handle_runtime_command(
        &mut state,
        &signal_tx,
        &event_tx,
        MediaCommand::Next {
            bus_name: TEST_PLAYER_NAME.to_string(),
        },
    )
    .await;

    assert_eq!(fixture.next_calls(), 1);
    assert!(state.delayed_refreshes.contains_key(TEST_PLAYER_NAME));
    for (_, task) in state.delayed_refreshes {
        task.abort();
    }
}

#[tokio::test]
async fn runtime_signal_refreshes_cache_publishes_and_schedules_fallback() {
    let fixture = MprisFixture::start().await;
    let player = build_player_state(&fixture.client, TEST_PLAYER_NAME, &MediaConfig::default())
        .await
        .expect("probe test MPRIS player")
        .expect("stable test MPRIS owner");
    let cancel_tx = player.listener_cancel.clone();
    let mut state = MediaRuntimeState::new();
    state.players.insert(TEST_PLAYER_NAME.to_string(), player);
    let (signal_tx, _signal_rx) = tokio::sync::mpsc::channel(4);
    let (event_tx, event_rx) = async_channel::bounded(4);

    // A native change refreshes the one named player before publishing
    handle_runtime_signal(
        &mut state,
        &signal_tx,
        &event_tx,
        MediaSignal::PropertiesChanged {
            bus_name: TEST_PLAYER_NAME.to_string(),
            origin: MediaRefreshOrigin::Bus,
        },
    )
    .await;

    assert!(state.cache.contains_key(TEST_PLAYER_NAME));
    assert!(state.delayed_refreshes.contains_key(TEST_PLAYER_NAME));
    let event = event_rx.recv().await.expect("published media snapshot");
    assert!(matches!(
        event,
        UiEvent::MediaUpdated(infos)
            if infos.as_slice()
                == [MediaInfo {
                    bus_name: TEST_PLAYER_NAME.to_string(),
                    identity: "UnixNotis Test Player".to_string(),
                    browser_family: None,
                    owner_pid: Some(std::process::id()),
                    title: String::new(),
                    artist: String::new(),
                    playback_status: "Playing".to_string(),
                    art_source: None,
                    can_play: true,
                    can_pause: true,
                    can_next: true,
                    can_prev: true,
                }]
    ));
    let _ = cancel_tx.send(true);
    for (_, task) in state.delayed_refreshes {
        task.abort();
    }
}
