use std::sync::atomic::Ordering;

use tracing::warn;

use crate::expire::ExpirationScheduler;

use super::DaemonState;

impl DaemonState {
    pub fn set_scheduler(&self, scheduler: ExpirationScheduler) {
        // Scheduler is wired once during daemon startup
        if self.scheduler.set(scheduler).is_err() {
            warn!("expiration scheduler was already installed; ignoring duplicate initialization");
            return;
        }
        self.scheduler_missing_warned.store(false, Ordering::SeqCst);
    }

    fn scheduler(&self) -> Option<ExpirationScheduler> {
        // Cloning the sender handle is cheap and keeps await points simple
        let scheduler = self.scheduler.get().cloned();
        if scheduler.is_none() && self.mark_missing_scheduler_warning_needed() {
            warn!("expiration scheduler is unavailable during live daemon operation");
        }
        scheduler
    }

    pub(in crate::daemon::state) fn mark_missing_scheduler_warning_needed(&self) -> bool {
        !self.scheduler_missing_warned.swap(true, Ordering::SeqCst)
    }

    pub(in crate::daemon) fn cancel_expiration(&self, id: u32) {
        // Missing scheduler means startup is still incomplete, so skip quietly
        let Some(scheduler) = self.scheduler() else {
            return;
        };
        scheduler.schedule(id, None);
    }

    pub fn cancel_expirations(&self, ids: &[u32]) {
        // Cancel timers for every removed active id so stale wakeups do not build up
        // Per-id cancel keeps the existing lazy heap design simple and predictable
        let Some(scheduler) = self.scheduler() else {
            return;
        };
        for id in ids {
            scheduler.schedule(*id, None);
        }
    }
}
