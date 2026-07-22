//! Expiration and timed-DND scheduler ownership for shared daemon state

use std::sync::atomic::Ordering;

use tokio::sync::MutexGuard;
use tracing::{debug, warn};

use crate::dnd_expiration::DndExpirationScheduler;
use crate::expire::ExpirationScheduler;
use crate::store::DndWrite;

use super::DaemonState;

const MAX_DND_DURATION_SECONDS: i64 = 366 * 24 * 60 * 60;

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

    pub(in crate::daemon) async fn apply_dnd_state(&self, enabled: bool) -> zbus::fdo::Result<()> {
        let _write_guard = self.lock_dnd_write().await;
        let write = {
            let mut store = self.store.lock().await;
            // The store records the previous revision so failed persistence can roll back safely
            store.set_dnd(enabled)
        };
        self.finalize_dnd_write(write).await
    }

    pub(in crate::daemon) async fn apply_dnd_until(
        &self,
        expires_at: i64,
    ) -> zbus::fdo::Result<()> {
        let _write_guard = self.lock_dnd_write().await;
        let now = chrono::Utc::now().timestamp();
        let duration = expires_at.saturating_sub(now);
        if duration <= 0 || duration > MAX_DND_DURATION_SECONDS {
            return Err(zbus::fdo::Error::InvalidArgs(
                "DND expiration must be within the next 366 days".to_string(),
            ));
        }
        let write = {
            let mut store = self.store.lock().await;
            store.set_dnd_until(expires_at)
        };
        self.finalize_dnd_write(write).await
    }

    pub(in crate::daemon) async fn apply_toggle_dnd(&self) -> zbus::fdo::Result<()> {
        let _write_guard = self.lock_dnd_write().await;
        let write = {
            let mut store = self.store.lock().await;
            // Toggle computation and mutation share one store revision
            store.toggle_dnd()
        };
        self.finalize_dnd_write(write).await
    }

    pub(crate) async fn apply_dnd_expiration(&self, expires_at: i64) -> zbus::fdo::Result<()> {
        let _write_guard = self.lock_dnd_write().await;
        let write = {
            let mut store = self.store.lock().await;
            // Stale timers cannot disable a newer timed or indefinite DND value
            store.expire_dnd_if_current(expires_at, chrono::Utc::now().timestamp())
        };
        self.finalize_dnd_write(write).await
    }

    async fn finalize_dnd_write(&self, write: DndWrite) -> zbus::fdo::Result<()> {
        if let Some(store) = write.persist.as_ref() {
            // Disk I/O stays outside the notification-store lock
            if let Err(error) = store.persist(write.current, write.current_expires_at) {
                warn!(?error, "failed to persist do-not-disturb state");
                let mut state = self.store.lock().await;
                let rolled_back = state.rollback_dnd_write_if_current(&write);
                if rolled_back {
                    debug!(
                        revision = write.revision,
                        current = write.current,
                        previous = write.previous,
                        "rolled back do-not-disturb state after persistence failure"
                    );
                } else {
                    debug!(
                        revision = write.revision,
                        current = write.current,
                        "skipped do-not-disturb rollback because newer state already exists"
                    );
                }
                return Err(zbus::fdo::Error::Failed(
                    "failed to persist do-not-disturb state".to_string(),
                ));
            }
        }
        if write.changed {
            // Timer and signal updates follow the durable state transition
            self.schedule_dnd_expiration(write.current_expires_at);
            if let Err(error) = self.publish_state_changed().await {
                warn!(
                    ?error,
                    "do-not-disturb state changed but post-commit signal fanout failed"
                );
            }
        }
        Ok(())
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
