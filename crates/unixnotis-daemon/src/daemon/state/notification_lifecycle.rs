use std::sync::Arc;

use tracing::warn;
use unixnotis_core::{CloseReason, Notification};

use super::DaemonState;

impl DaemonState {
    pub async fn close_notification(&self, id: u32, reason: CloseReason) -> zbus::Result<()> {
        let removed = {
            let mut store = self.store.lock().await;
            let removed = store.close(id, reason);
            if let Some(notification) = removed.as_ref() {
                // Cancellation is ordered before a replacement can acquire the store lock
                self.cancel_expiration(notification.key());
            }
            removed
        };
        if removed.is_none() {
            return Ok(());
        }
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
            let outcome = store.dismiss_from_panel(id);
            if let Some(key) = outcome.removed_active {
                self.cancel_expiration(key);
            }
            outcome
        };

        if !outcome.removed_any() {
            return Ok(());
        }

        if let Err(err) = self
            .publish_notification_dismissed(id, outcome.removed_active.is_some())
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
            let outcome = store.dismiss_replied_generation(id, expected);
            if let Some(key) = outcome.removed_active {
                self.cancel_expiration(key);
            }
            outcome
        };
        if !outcome.removed_any() {
            return Ok(false);
        }

        if let Err(err) = self
            .publish_notification_dismissed(id, outcome.removed_active.is_some())
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
