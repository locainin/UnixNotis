use std::time::Instant;
use tracing::debug;
use unixnotis_core::CloseReason;
use zbus::message::Header;

use super::NotificationServer;
use crate::daemon::notifications::identity::resolve_sender_metadata;
use crate::daemon::notifications::ingress::metrics::RejectedRequest;

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
        if !self
            .close_quota
            .admit_principal(super::quota_principal(&sender), Instant::now())
        {
            let rejected = self
                .ingress_metrics
                .record_rejection(RejectedRequest::CloseQuota);
            debug!(rejected, "close request rejected by principal quota");
            return Err(zbus::fdo::Error::LimitsExceeded(
                "notification close quota exceeded".to_string(),
            ));
        }
        let removed = {
            let mut store = self.state.store.lock().await;
            // Ownership and removal share one lock so a same-ID replacement cannot race the close
            store.close_owned_active(
                id,
                sender.sender_name.as_deref(),
                sender.sender_pid,
                sender.sender_start_time,
                CloseReason::ClosedByCall,
            )
        };
        let Some(removed) = removed else {
            debug!(
                id,
                sender = sender.sender_name.as_deref().unwrap_or("unknown"),
                sender_pid = sender.sender_pid,
                "notification close target is not closable"
            );
            // Missing, foreign, historical, and otherwise non-closable IDs are indistinguishable
            return Err(generic_close_error());
        };

        self.state.cancel_expiration(removed.key());
        self.state
            .publish_notification_closed(removed.key(), CloseReason::ClosedByCall)
            .await
            .map_err(crate::daemon::to_fdo_error)
    }
}

const fn generic_close_error() -> zbus::fdo::Error {
    // One empty generic failure prevents existence and ownership disclosure
    zbus::fdo::Error::Failed(String::new())
}
