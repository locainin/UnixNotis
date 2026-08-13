//! Allocation-free counters for notification ingress pressure

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::daemon::notifications) enum RejectedRequest {
    NotifyQuota,
    NotifyConcurrency,
    CloseQuota,
}

pub(in crate::daemon::notifications) struct IngressMetrics {
    notify_quota_rejections: AtomicU64,
    notify_concurrency_rejections: AtomicU64,
    close_quota_rejections: AtomicU64,
    active_handlers: AtomicUsize,
    peak_active_handlers: AtomicUsize,
}

pub(in crate::daemon::notifications) struct ActiveHandler<'a> {
    metrics: &'a IngressMetrics,
}

impl IngressMetrics {
    pub(in crate::daemon::notifications) const fn new() -> Self {
        Self {
            notify_quota_rejections: AtomicU64::new(0),
            notify_concurrency_rejections: AtomicU64::new(0),
            close_quota_rejections: AtomicU64::new(0),
            active_handlers: AtomicUsize::new(0),
            peak_active_handlers: AtomicUsize::new(0),
        }
    }

    pub(in crate::daemon::notifications) fn record_rejection(
        &self,
        rejected: RejectedRequest,
    ) -> u64 {
        let counter = match rejected {
            RejectedRequest::NotifyQuota => &self.notify_quota_rejections,
            RejectedRequest::NotifyConcurrency => &self.notify_concurrency_rejections,
            RejectedRequest::CloseQuota => &self.close_quota_rejections,
        };
        counter.fetch_add(1, Ordering::Relaxed).saturating_add(1)
    }

    pub(in crate::daemon::notifications) fn enter_handler(&self) -> ActiveHandler<'_> {
        let active = self
            .active_handlers
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        // Atomic maximum records concurrency peaks without locks or retry-loop bookkeeping
        self.peak_active_handlers
            .fetch_max(active, Ordering::Relaxed);
        ActiveHandler { metrics: self }
    }
}

impl Drop for ActiveHandler<'_> {
    fn drop(&mut self) {
        self.metrics.active_handlers.fetch_sub(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
#[path = "tests/metrics.rs"]
mod tests;
