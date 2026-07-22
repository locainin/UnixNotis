//! Expiration and timed-DND scheduler ownership for shared daemon state

use std::sync::atomic::Ordering;

use tokio::sync::MutexGuard;
use tracing::warn;

use crate::dnd_expiration::DndExpirationScheduler;
use crate::expire::ExpirationScheduler;

use super::DaemonState;

impl DaemonState {
    pub(in crate::daemon) async fn lock_dnd_write(&self) -> MutexGuard<'_, ()> {
        // One writer keeps disk state and the scheduled deadline in the same order
        self.dnd_write_lock.lock().await
    }

    pub fn set_dnd_scheduler(&self, scheduler: DndExpirationScheduler) {
        if self.dnd_scheduler.set(scheduler).is_err() {
            warn!("DND scheduler was already installed; ignoring duplicate initialization");
            return;
        }
        self.dnd_scheduler_missing_warned
            .store(false, Ordering::SeqCst);
    }

    pub(crate) fn schedule_dnd_expiration(&self, expires_at: Option<i64>) {
        let Some(scheduler) = self.dnd_scheduler.get() else {
            if !self
                .dnd_scheduler_missing_warned
                .swap(true, Ordering::SeqCst)
            {
                warn!("DND scheduler is unavailable during live daemon operation");
            }
            return;
        };
        scheduler.schedule(expires_at);
    }

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
        // Per-id cancel keeps the lazy expiration heap bounded without rebuilding it here
        let Some(scheduler) = self.scheduler() else {
            return;
        };
        for id in ids {
            scheduler.schedule(*id, None);
        }
    }
}
