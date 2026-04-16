//! DND mutation and persistence helpers for ControlServer
//!
//! Keeps toggle/set flow and guarded rollback logic out of the main interface file

use crate::store::DndWrite;
use tracing::{debug, warn};

use super::ControlServer;

impl ControlServer {
    pub(super) async fn apply_dnd_state(&self, enabled: bool) -> zbus::fdo::Result<()> {
        let write = {
            let mut store = self.state.store.lock().await;
            // Set request mutates once under lock and records rollback guards
            store.set_dnd(enabled)
        };
        self.finalize_dnd_write(write).await
    }

    pub(super) async fn apply_toggle_dnd(&self) -> zbus::fdo::Result<()> {
        let write = {
            let mut store = self.state.store.lock().await;
            // Toggle computation and write stay in one critical section
            store.toggle_dnd()
        };
        self.finalize_dnd_write(write).await
    }

    async fn finalize_dnd_write(&self, write: DndWrite) -> zbus::fdo::Result<()> {
        if let Some(store) = write.persist.as_ref() {
            // Persist outside the main store lock to avoid blocking notify paths on I/O
            if let Err(err) = store.persist(write.current) {
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
            // Mutation is already committed; signal fanout is best-effort.
            if let Err(err) = self.state.emit_state_changed().await {
                warn!(
                    ?err,
                    "do-not-disturb state changed but post-commit signal fanout failed"
                );
            }
        }
        Ok(())
    }
}
