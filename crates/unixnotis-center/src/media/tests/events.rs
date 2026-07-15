use super::{
    merge_mode_for_signal, owner_is_unchanged, owner_rebuild_outcome,
    replacement_removal_needs_snapshot, should_publish_immediate_command_snapshot,
    should_schedule_metadata_fallback, MediaCacheMergeMode, OwnerChangeOutcome,
};
use crate::media::{MediaCommand, MediaRefreshOrigin};

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
    // Outcome classification depends only on whether a rebuilt state exists
    let outcome = owner_rebuild_outcome(true);

    assert_eq!(outcome, OwnerChangeOutcome::Applied);
    assert!(!replacement_removal_needs_snapshot(true, outcome));
}

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
