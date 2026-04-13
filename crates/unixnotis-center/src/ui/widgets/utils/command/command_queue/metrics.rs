//! Queue pressure counters and debug summaries

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

#[derive(Default)]
pub(super) struct CommandQueueMetrics {
    // All jobs handed to the queue layer
    enqueued: AtomicU64,
    // Slow jobs that entered the jitter queue
    delayed: AtomicU64,
    // Slow jobs that skipped jitter because that queue was full
    delayed_bypassed: AtomicU64,
    // Action jobs that overflowed into the fallback worker
    action_overflow: AtomicU64,
    // Refresh jobs that overflowed into the coalescer
    refresh_overflow: AtomicU64,
    // Action jobs dropped after both queues filled
    action_dropped: AtomicU64,
    // Refresh jobs that replaced an older job with the same key
    refresh_replaced: AtomicU64,
    // Refresh jobs that evicted the oldest pending key
    refresh_evicted: AtomicU64,
}

#[derive(Clone, Copy, Default)]
pub(super) struct CommandQueueMetricsSnapshot {
    pub(super) enqueued: u64,
    pub(super) delayed: u64,
    pub(super) delayed_bypassed: u64,
    pub(super) action_overflow: u64,
    pub(super) refresh_overflow: u64,
    pub(super) action_dropped: u64,
    pub(super) refresh_replaced: u64,
    pub(super) refresh_evicted: u64,
}

impl CommandQueueMetrics {
    pub(super) fn global() -> &'static Self {
        static METRICS: OnceLock<CommandQueueMetrics> = OnceLock::new();
        METRICS.get_or_init(Self::default)
    }

    pub(super) fn record_enqueued(&self) {
        self.enqueued.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn record_delayed(&self) {
        self.delayed.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn record_delayed_bypassed(&self) {
        self.delayed_bypassed.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn record_action_overflow(&self) {
        self.action_overflow.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn record_refresh_overflow(&self) {
        self.refresh_overflow.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn record_action_dropped(&self) {
        self.action_dropped.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn record_refresh_replaced(&self) {
        self.refresh_replaced.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn record_refresh_evicted(&self) {
        self.refresh_evicted.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn snapshot(&self) -> CommandQueueMetricsSnapshot {
        CommandQueueMetricsSnapshot {
            enqueued: self.enqueued.load(Ordering::Relaxed),
            delayed: self.delayed.load(Ordering::Relaxed),
            delayed_bypassed: self.delayed_bypassed.load(Ordering::Relaxed),
            action_overflow: self.action_overflow.load(Ordering::Relaxed),
            refresh_overflow: self.refresh_overflow.load(Ordering::Relaxed),
            action_dropped: self.action_dropped.load(Ordering::Relaxed),
            refresh_replaced: self.refresh_replaced.load(Ordering::Relaxed),
            refresh_evicted: self.refresh_evicted.load(Ordering::Relaxed),
        }
    }
}

impl CommandQueueMetricsSnapshot {
    pub(super) fn summary(self) -> String {
        format!(
            "enqueued={} delayed={} delayed_bypassed={} action_overflow={} refresh_overflow={} action_dropped={} refresh_replaced={} refresh_evicted={}",
            self.enqueued,
            self.delayed,
            self.delayed_bypassed,
            self.action_overflow,
            self.refresh_overflow,
            self.action_dropped,
            self.refresh_replaced,
            self.refresh_evicted
        )
    }
}
