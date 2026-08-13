//! Notification ingress metric tests

use std::sync::atomic::Ordering;

use super::{IngressMetrics, RejectedRequest};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IngressMetricsSnapshot {
    notify_quota_rejections: u64,
    notify_concurrency_rejections: u64,
    close_quota_rejections: u64,
    active_handlers: usize,
    peak_active_handlers: usize,
}

fn snapshot(metrics: &IngressMetrics) -> IngressMetricsSnapshot {
    IngressMetricsSnapshot {
        notify_quota_rejections: metrics.notify_quota_rejections.load(Ordering::Relaxed),
        notify_concurrency_rejections: metrics
            .notify_concurrency_rejections
            .load(Ordering::Relaxed),
        close_quota_rejections: metrics.close_quota_rejections.load(Ordering::Relaxed),
        active_handlers: metrics.active_handlers.load(Ordering::Relaxed),
        peak_active_handlers: metrics.peak_active_handlers.load(Ordering::Relaxed),
    }
}

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

    let snapshot = snapshot(&metrics);
    assert_eq!(snapshot.notify_quota_rejections, 2);
    assert_eq!(snapshot.notify_concurrency_rejections, 1);
    assert_eq!(snapshot.close_quota_rejections, 1);
}

#[test]
fn handler_guard_tracks_current_and_peak_concurrency_without_leaking_activity() {
    let metrics = IngressMetrics::new();

    let first = metrics.enter_handler();
    let second = metrics.enter_handler();
    assert_eq!(snapshot(&metrics).active_handlers, 2);
    assert_eq!(snapshot(&metrics).peak_active_handlers, 2);
    drop(second);
    assert_eq!(snapshot(&metrics).active_handlers, 1);
    drop(first);

    let snapshot = snapshot(&metrics);
    assert_eq!(snapshot.active_handlers, 0);
    assert_eq!(snapshot.peak_active_handlers, 2);
}
