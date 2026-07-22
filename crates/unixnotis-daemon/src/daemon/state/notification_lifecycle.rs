use std::sync::Arc;

use tracing::warn;
use unixnotis_core::{CloseReason, Notification};

use super::DaemonState;

impl DaemonState {
    pub async fn close_notification(&self, id: u32, reason: CloseReason) -> zbus::Result<()> {
        let removed = {
            let mut store = self.store.lock().await;
            store.close(id, reason)
        };
        if removed.is_none() {
            return Ok(());
        }
        // Timer cancel happens before signal fanout so stale wakeups stop right away
        self.cancel_expiration(id);

        if let Err(err) = self.publish_notification_closed(id, reason).await {
            warn!(
                ?err,
                id,
                reason = reason as u32,
                "notification close committed but one or more D-Bus signals failed"
            );
        }
        Ok(())
    }

    pub async fn dismiss_from_panel(&self, id: u32) -> zbus::Result<()> {
        let outcome = {
            let mut store = self.store.lock().await;
            store.dismiss_from_panel(id)
        };

        if !outcome.removed_any() {
            return Ok(());
        }

        if outcome.removed_active {
            // Panel dismiss removes the active entry, so its timer must go too
            self.cancel_expiration(id);
        }
        if let Err(err) = self
            .publish_notification_dismissed(id, outcome.removed_active)
            .await
        {
            warn!(
                ?err,
                id, "panel dismiss committed but one or more D-Bus signals failed"
            );
        }
        Ok(())
    }

    pub async fn dismiss_replied_if_current(
        &self,
        id: u32,
        expected: &Arc<Notification>,
    ) -> zbus::Result<bool> {
        let outcome = {
            // Object identity prevents an older action from deleting a same-ID replacement
            let mut store = self.store.lock().await;
            store.dismiss_replied_generation(id, expected)
        };
        if !outcome.removed_any() {
            return Ok(false);
        }

        if outcome.removed_active {
            // Only the matching active generation owns this expiration timer
            self.cancel_expiration(id);
        }
        if let Err(err) = self
            .publish_notification_dismissed(id, outcome.removed_active)
            .await
        {
            warn!(
                ?err,
                id, "generation-safe dismiss committed but one or more D-Bus signals failed"
            );
        }
        Ok(true)
    }
}
