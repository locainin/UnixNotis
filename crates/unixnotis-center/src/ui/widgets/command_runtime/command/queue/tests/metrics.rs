use super::CommandQueueMetrics;

#[test]
fn snapshot_reports_each_recorded_queue_event() {
    let metrics = CommandQueueMetrics::default();

    metrics.record_enqueued();
    metrics.record_delayed();
    metrics.record_delayed_bypassed();
    metrics.record_action_overflow();
    metrics.record_refresh_overflow();
    metrics.record_action_dropped();
    metrics.record_refresh_replaced();
    metrics.record_refresh_evicted();
    let snapshot = metrics.snapshot();

    assert_eq!(snapshot.enqueued, 1);
    assert_eq!(snapshot.delayed, 1);
    assert_eq!(snapshot.delayed_bypassed, 1);
    assert_eq!(snapshot.action_overflow, 1);
    assert_eq!(snapshot.refresh_overflow, 1);
    assert_eq!(snapshot.action_dropped, 1);
    assert_eq!(snapshot.refresh_replaced, 1);
    assert_eq!(snapshot.refresh_evicted, 1);
}
