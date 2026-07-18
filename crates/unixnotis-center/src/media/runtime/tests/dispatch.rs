use super::super::cache::MediaCacheMergeMode;
use super::super::dispatch::{
    merge_mode_for_signal, should_publish_immediate_command_snapshot,
    should_schedule_metadata_fallback,
};
use super::super::{MediaRefreshOrigin, MediaSignal};
use crate::media::MediaCommand;

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
