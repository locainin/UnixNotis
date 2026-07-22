use super::super::notifications::clear_all_signal_plan;
use crate::test_support::daemon_state_for_test;

#[test]
fn clear_all_with_no_active_rows_still_invalidates_snapshot() {
    let plan = clear_all_signal_plan(&[]);

    // No live rows means there is nothing to close
    assert!(!plan.publish_close_signals);
    // Empty clear is still the escape hatch for stale client rows
    assert!(plan.publish_snapshot_invalidated);
    // State refresh still needs a chance to run
    assert!(plan.publish_state_changed);
}

#[test]
fn clear_all_with_active_rows_keeps_close_fanout_and_refresh() {
    let plan = clear_all_signal_plan(&[11, 12]);

    // Active rows still need the normal close signals
    assert!(plan.publish_close_signals);
    // Clients still need a full refresh after the clear
    assert!(plan.publish_snapshot_invalidated);
    assert!(plan.publish_state_changed);
}

#[test]
fn clear_all_signal_plan_treats_any_non_empty_id_set_as_close_fanout() {
    let plan = clear_all_signal_plan(&[99]);

    // A single active row still needs both freedesktop and control close fanout
    assert!(plan.publish_close_signals);
    assert!(plan.publish_snapshot_invalidated);
    assert!(plan.publish_state_changed);
}

#[tokio::test]
async fn clear_all_without_ids_still_refreshes_cached_control_state() {
    let state = daemon_state_for_test(false).await;

    state.publish_notifications_cleared(Vec::new()).await;

    // A no-row clear still refreshes state caches so clients can recover stale views
    assert!(state
        .events
        .last_state
        .lock()
        .expect("state cache lock")
        .is_some());
    assert!(state
        .events
        .last_popup_gate
        .lock()
        .expect("popup gate cache lock")
        .is_some());
}
