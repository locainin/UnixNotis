use tracing::warn;
use unixnotis_core::CloseReason;

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

        if let Err(err) = self.emit_close_fanout(id, reason).await {
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
        if let Err(err) = self.emit_dismiss_fanout(id, outcome.removed_active).await {
            warn!(
                ?err,
                id, "panel dismiss committed but one or more D-Bus signals failed"
            );
        }
        Ok(())
    }
}
