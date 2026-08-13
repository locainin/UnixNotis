use tracing::warn;
use unixnotis_core::CloseReason;

use crate::daemon::DaemonState;

impl DaemonState {
    pub(crate) async fn close_notification(
        &self,
        id: u32,
        reason: CloseReason,
    ) -> zbus::Result<()> {
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
        if let Err(error) = self
            .publish_notification_closed(removed.key(), reason)
            .await
        {
            warn!(
                ?error,
                id,
                reason = reason as u32,
                "notification close committed but one or more D-Bus signals failed"
            );
        }
        Ok(())
    }
}
