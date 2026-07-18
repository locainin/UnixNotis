use std::time::Duration;

use super::super::owner::OwnerChangeOutcome;
use super::super::r#loop::{
    owner_change_needs_retry, send_owner_rebuild_retry_after, OWNER_REBUILD_RETRY_MS,
};

#[test]
fn only_unstable_owner_outcome_schedules_rebuild_retry() {
    assert!(owner_change_needs_retry(OwnerChangeOutcome::RetryNeeded));
    assert!(!owner_change_needs_retry(OwnerChangeOutcome::Applied));
    assert!(!owner_change_needs_retry(OwnerChangeOutcome::Removed));
}

#[tokio::test]
async fn owner_rebuild_retry_emits_one_delayed_refresh_signal() {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);

    send_owner_rebuild_retry_after(Duration::from_millis(1), sender).await;

    // A missing retry is a focused failure rather than an unbounded channel wait
    let retry = tokio::time::timeout(Duration::from_secs(2), receiver.recv())
        .await
        .expect("owner rebuild retry should arrive promptly");
    assert_eq!(retry, Some(()));
    assert!(receiver.try_recv().is_err(), "retry must emit only once");
    assert_eq!(OWNER_REBUILD_RETRY_MS, 200);
}
