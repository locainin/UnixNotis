//! Timed DND scheduler ownership for shared daemon state

use std::sync::atomic::Ordering;

use tokio::sync::MutexGuard;
use tracing::warn;

use crate::dnd_expiration::DndExpirationScheduler;

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
}
