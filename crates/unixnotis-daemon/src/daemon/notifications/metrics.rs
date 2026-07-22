//! Allocation-free counters for notification ingress pressure

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RejectedRequest {
    NotifyQuota,
    NotifyConcurrency,
    CloseQuota,
}

pub(super) struct IngressMetrics {
    notify_quota_rejections: AtomicU64,
    notify_concurrency_rejections: AtomicU64,
    close_quota_rejections: AtomicU64,
    active_handlers: AtomicUsize,
    peak_active_handlers: AtomicUsize,
}

pub(super) struct ActiveHandler<'a> {
    metrics: &'a IngressMetrics,
}

impl IngressMetrics {
    pub(super) const fn new() -> Self {
        Self {
            notify_quota_rejections: AtomicU64::new(0),
            notify_concurrency_rejections: AtomicU64::new(0),
            close_quota_rejections: AtomicU64::new(0),
            active_handlers: AtomicUsize::new(0),
            peak_active_handlers: AtomicUsize::new(0),
        }
    }

    pub(super) fn record_rejection(&self, rejected: RejectedRequest) -> u64 {
        let counter = match rejected {
            RejectedRequest::NotifyQuota => &self.notify_quota_rejections,
            RejectedRequest::NotifyConcurrency => &self.notify_concurrency_rejections,
            RejectedRequest::CloseQuota => &self.close_quota_rejections,
        };
        counter.fetch_add(1, Ordering::Relaxed).saturating_add(1)
    }

    pub(super) fn enter_handler(&self) -> ActiveHandler<'_> {
        let active = self
            .active_handlers
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        // A compare loop works on every supported Rust release and never lowers the peak
        let mut peak = self.peak_active_handlers.load(Ordering::Relaxed);
        while active > peak {
            match self.peak_active_handlers.compare_exchange_weak(
                peak,
                active,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(observed) => peak = observed,
            }
        }
        ActiveHandler { metrics: self }
    }

    #[cfg(test)]
    pub(super) fn snapshot(&self) -> IngressMetricsSnapshot {
        IngressMetricsSnapshot {
            notify_quota_rejections: self.notify_quota_rejections.load(Ordering::Relaxed),
            notify_concurrency_rejections: self
                .notify_concurrency_rejections
                .load(Ordering::Relaxed),
            close_quota_rejections: self.close_quota_rejections.load(Ordering::Relaxed),
            active_handlers: self.active_handlers.load(Ordering::Relaxed),
            peak_active_handlers: self.peak_active_handlers.load(Ordering::Relaxed),
        }
    }
}

impl Drop for ActiveHandler<'_> {
    fn drop(&mut self) {
        self.metrics.active_handlers.fetch_sub(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct IngressMetricsSnapshot {
    pub(super) notify_quota_rejections: u64,
    pub(super) notify_concurrency_rejections: u64,
    pub(super) close_quota_rejections: u64,
    pub(super) active_handlers: usize,
    pub(super) peak_active_handlers: usize,
}

#[cfg(test)]
#[path = "tests/metrics.rs"]
mod tests;
