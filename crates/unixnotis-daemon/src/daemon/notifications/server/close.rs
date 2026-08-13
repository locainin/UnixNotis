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

        // Unauthorized close targets collapse into one generic protocol failure
        let sender = resolve_sender_metadata(
            &self.state.sender_metadata_cache,
            self.state.connection(),
            header,
        )
        .await;
        let principal = super::quota_principal(&sender);
        if !self
            .close_quota
            .try_admit_close_attempt(principal, Instant::now())
            .is_allowed()
        {
            let rejected = self
                .ingress_metrics
                .record_rejection(RejectedRequest::CloseQuota);
            debug!(rejected, "close request rejected by principal quota");
            return Err(zbus::fdo::Error::LimitsExceeded(
                "notification close quota exceeded".to_string(),
            ));
        }
        // Replacement commits share this gate so one close request targets one generation
        let _interaction = self.state.interaction_gates.lock(id).await;
        let removed = {
            let mut store = self.state.store.lock().await;
            let authorization = store.close_authorization(
                id,
                sender.sender_name.as_deref(),
                sender.sender_pid,
                sender.sender_start_time,
            );
            let crate::store::CloseAuthorization::OwnedActive(expected) = authorization else {
                debug!(
                    id,
                    sender = sender.sender_name.as_deref().unwrap_or("unknown"),
                    sender_pid = sender.sender_pid,
                    "notification close target is not closable"
                );
                // Invalid attempts charge only their caller and never consume shared mutation capacity
                return Err(generic_close_error());
            };
            if !self
                .close_quota
                .try_admit_close_commit(Instant::now())
                .is_allowed()
            {
                let rejected = self
                    .ingress_metrics
                    .record_rejection(RejectedRequest::CloseQuota);
                debug!(rejected, "owned close rejected by global commit quota");
                return Err(zbus::fdo::Error::LimitsExceeded(
                    "notification close quota exceeded".to_string(),
                ));
            }

            // Admission and removal share one store lock so only a real mutation spends global quota
            store.close_owned_active_generation(
                expected,
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
            // A concurrent replacement or close stays indistinguishable from every invalid target
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
