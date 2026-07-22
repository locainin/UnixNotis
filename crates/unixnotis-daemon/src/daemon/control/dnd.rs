//! DND mutation and persistence helpers for `ControlServer`
//!
//! Keeps toggle/set flow and guarded rollback logic out of the main interface file

use crate::store::DndWrite;
use tracing::{debug, warn};

use super::ControlServer;

const MAX_DND_DURATION_SECONDS: i64 = 366 * 24 * 60 * 60;

impl ControlServer {
    pub(super) async fn apply_dnd_state(&self, enabled: bool) -> zbus::fdo::Result<()> {
        let _write_guard = self.state.lock_dnd_write().await;
        let write = {
            let mut store = self.state.store.lock().await;
            // Set request mutates once under lock and records rollback guards
            store.set_dnd(enabled)
        };
        self.finalize_dnd_write(write).await
    }

    pub(super) async fn apply_dnd_until(&self, expires_at: i64) -> zbus::fdo::Result<()> {
        let _write_guard = self.state.lock_dnd_write().await;
        let now = chrono::Utc::now().timestamp();
        let duration = expires_at.saturating_sub(now);
        if duration <= 0 || duration > MAX_DND_DURATION_SECONDS {
            return Err(zbus::fdo::Error::InvalidArgs(
                "DND expiration must be within the next 366 days".to_string(),
            ));
        }
        let write = {
            let mut store = self.state.store.lock().await;
            store.set_dnd_until(expires_at)
        };
        self.finalize_dnd_write(write).await
    }

    pub(super) async fn apply_toggle_dnd(&self) -> zbus::fdo::Result<()> {
        let _write_guard = self.state.lock_dnd_write().await;
        let write = {
            let mut store = self.state.store.lock().await;
            // Toggle computation and write stay in one critical section
            store.toggle_dnd()
        };
        self.finalize_dnd_write(write).await
    }

    pub(crate) async fn apply_dnd_expiration(&self, expires_at: i64) -> zbus::fdo::Result<()> {
        let _write_guard = self.state.lock_dnd_write().await;
        let write = {
            let mut store = self.state.store.lock().await;
            // The store rejects stale deadlines that were replaced while the task slept
            store.expire_dnd_if_current(expires_at, chrono::Utc::now().timestamp())
        };
        self.finalize_dnd_write(write).await
    }

    async fn finalize_dnd_write(&self, write: DndWrite) -> zbus::fdo::Result<()> {
        if let Some(store) = write.persist.as_ref() {
            // Persist outside the main store lock to avoid blocking notify paths on I/O
            if let Err(err) = store.persist(write.current, write.current_expires_at) {
                warn!(?err, "failed to persist do-not-disturb state");
                // Only rollback if this failing write is still the latest in-memory value
                let mut state = self.state.store.lock().await;
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
            // Scheduling follows durable commit so failed writes keep the previous timer
            self.state.schedule_dnd_expiration(write.current_expires_at);
            // Mutation is already committed; signal fanout is best-effort
            if let Err(err) = self.state.publish_state_changed().await {
                warn!(
                    ?err,
                    "do-not-disturb state changed but post-commit signal fanout failed"
                );
            }
        }
        Ok(())
    }
}
