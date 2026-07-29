use std::sync::Arc;

use tracing::warn;
use unixnotis_core::{CloseReason, Notification, NotificationKey};

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
        let Some(removed) = removed else {
            return Ok(());
        };
        if let Err(err) = self
            .publish_notification_closed(removed.key(), reason)
            .await
        {
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

        let removed_active = outcome.removed_active.is_some();
        let key = outcome
            .removed_active
            .or(outcome.removed_history)
            .expect("a removed notification must retain its generation");
        if let Err(err) = self
            .publish_notification_dismissed(key, removed_active)
            .await
        {
            warn!(
                ?err,
                id, "panel dismiss committed but one or more D-Bus signals failed"
            );
        }
        Ok(())
    }

    pub async fn dismiss_generation(&self, key: NotificationKey) -> zbus::Result<()> {
        let outcome = {
            let mut store = self.store.lock().await;
            let outcome = store.dismiss_generation(key);
            if let Some(removed) = outcome.removed_active {
                self.cancel_expiration(removed);
            }
            outcome
        };

        if !outcome.removed_any() {
            return Err(zbus::Error::Failure(
                "notification generation is no longer current".to_string(),
            ));
        }

        let removed_active = outcome.removed_active.is_some();
        let removed = outcome
            .removed_active
            .or(outcome.removed_history)
            .expect("a removed generation must retain its exact key");
        if let Err(err) = self
            .publish_notification_dismissed(removed, removed_active)
            .await
        {
            warn!(
                ?err,
                id = key.id,
                generation = key.generation,
                "generation-safe dismiss committed but one or more D-Bus signals failed"
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

        let removed_active = outcome.removed_active.is_some();
        let key = outcome
            .removed_active
            .or(outcome.removed_history)
            .expect("a removed reply target must retain its generation");
        if let Err(err) = self
            .publish_notification_dismissed(key, removed_active)
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
