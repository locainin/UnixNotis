//! Notification ingress metric tests

use super::{IngressMetrics, RejectedRequest};

#[test]
fn rejection_counters_are_kept_separate_by_request_path() {
    let metrics = IngressMetrics::new();

    assert_eq!(metrics.record_rejection(RejectedRequest::NotifyQuota), 1);
    assert_eq!(metrics.record_rejection(RejectedRequest::NotifyQuota), 2);
    assert_eq!(
        metrics.record_rejection(RejectedRequest::NotifyConcurrency),
        1
    );
    assert_eq!(metrics.record_rejection(RejectedRequest::CloseQuota), 1);

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.notify_quota_rejections, 2);
    assert_eq!(snapshot.notify_concurrency_rejections, 1);
    assert_eq!(snapshot.close_quota_rejections, 1);
}

#[test]
fn handler_guard_tracks_current_and_peak_concurrency_without_leaking_activity() {
    let metrics = IngressMetrics::new();

    let first = metrics.enter_handler();
    let second = metrics.enter_handler();
    assert_eq!(metrics.snapshot().active_handlers, 2);
    assert_eq!(metrics.snapshot().peak_active_handlers, 2);
    drop(second);
    assert_eq!(metrics.snapshot().active_handlers, 1);
    drop(first);

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.active_handlers, 0);
    assert_eq!(snapshot.peak_active_handlers, 2);
}
