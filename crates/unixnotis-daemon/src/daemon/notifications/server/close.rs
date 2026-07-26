use tracing::debug;
use unixnotis_core::CloseReason;
use zbus::message::Header;

use crate::daemon::to_fdo_error;

use super::NotificationServer;
use crate::daemon::notifications::identity::resolve_sender_metadata;

impl NotificationServer {
    pub(super) async fn close_notification_if_owned(
        &self,
        id: u32,
        header: &Header<'_>,
    ) -> zbus::fdo::Result<()> {
        debug!(id, "close notification requested");

        // Close requests are ownership checked and become no-op when unauthorized
        let sender = resolve_sender_metadata(
            &self.state.sender_metadata_cache,
            self.state.connection(),
            header,
        )
        .await;
        let Some(sender_name) = sender.sender_name.as_deref() else {
            return Ok(());
        };

        let owned = {
            let store = self.state.store.lock().await;
            // Ownership check allows reconnect-safe close by same sender pid
            store.is_notification_owned_by(
                id,
                sender_name,
                sender.sender_pid,
                sender.sender_start_time,
            )
        };
        if !owned {
            debug!(
                id,
                sender = sender_name,
                sender_pid = sender.sender_pid,
                "ignoring close for unowned notification"
            );
            return Ok(());
        }

        self.state
            .close_notification(id, CloseReason::ClosedByCall)
            .await
            .map_err(to_fdo_error)
    }
}
