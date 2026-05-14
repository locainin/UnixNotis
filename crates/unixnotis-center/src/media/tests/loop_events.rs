use super::{
    merge_mode_for_signal, should_publish_immediate_command_snapshot,
    should_schedule_metadata_fallback, MediaCacheMergeMode,
};
use crate::media::{MediaCommand, MediaRefreshOrigin};

#[test]
fn fallback_generated_refreshes_do_not_rearm_followup_sweeps() {
    assert!(!should_schedule_metadata_fallback(
        MediaRefreshOrigin::Fallback
    ));
}

#[test]
fn bus_generated_refreshes_still_allow_one_bounded_followup_sweep() {
    assert!(should_schedule_metadata_fallback(MediaRefreshOrigin::Bus));
}

#[test]
fn skip_commands_wait_for_followup_refreshes() {
    assert!(!should_publish_immediate_command_snapshot(
        &MediaCommand::Next {
            bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
        }
    ));
    assert!(!should_publish_immediate_command_snapshot(
        &MediaCommand::Previous {
            bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
        }
    ));
}

#[test]
fn play_pause_still_refreshes_immediately() {
    assert!(should_publish_immediate_command_snapshot(
        &MediaCommand::PlayPause {
            bus_name: "org.mpris.MediaPlayer2.spotify".to_string(),
        }
    ));
}

#[test]
fn bus_updates_use_transition_merge_but_fallbacks_commit_final_state() {
    assert_eq!(
        merge_mode_for_signal(MediaRefreshOrigin::Bus),
        MediaCacheMergeMode::Transitioning
    );
    assert_eq!(
        merge_mode_for_signal(MediaRefreshOrigin::Fallback),
        MediaCacheMergeMode::Stable
    );
}
