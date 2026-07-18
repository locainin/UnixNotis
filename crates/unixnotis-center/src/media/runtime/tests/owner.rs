use super::super::owner::{
    owner_is_unchanged, owner_rebuild_outcome, replacement_removal_needs_snapshot,
    OwnerChangeOutcome,
};

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
