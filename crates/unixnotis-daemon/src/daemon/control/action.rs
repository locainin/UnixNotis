//! Validation for application action signals requested by trusted control clients

use std::future::Future;

use unixnotis_core::NotificationKey;
use zbus::SignalContext;

use crate::daemon::notifications::identity::resolve_callback_destination;
use crate::daemon::{to_fdo_error, NotificationServer, NOTIFICATIONS_OBJECT_PATH};

use super::ControlServer;

impl ControlServer {
    pub(super) async fn invoke_validated_action_generation(
        &self,
        notification: NotificationKey,
        action_key: &str,
        confirmed: bool,
    ) -> zbus::fdo::Result<()> {
        self.invoke_validated_action_generation_with_pre_emit(
            notification,
            action_key,
            confirmed,
            || std::future::ready(()),
        )
        .await
    }

    pub(super) async fn invoke_validated_action_generation_with_pre_emit<F, Fut>(
        &self,
        notification: NotificationKey,
        action_key: &str,
        confirmed: bool,
        pre_emit: F,
    ) -> zbus::fdo::Result<()>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = ()>,
    {
        // The guard spans validation, destination lookup, signal delivery, and exact cleanup
        // A same-ID replacement cannot commit while an ID-only protocol signal is in flight
        let _interaction = self.state.interaction_gates.lock(notification.id).await;
        let target = {
            // Capture one concrete generation while validating the stored action identity
            let store = self.state.store.lock().await;
            store
                .active_action_target_generation(notification, action_key, confirmed)
                .ok_or_else(|| {
                    zbus::fdo::Error::InvalidArgs(
                        "notification is not live or does not advertise this action".to_string(),
                    )
                })?
        };
        let bus_name = resolve_callback_destination(
            &self.state.sender_metadata_cache,
            self.state.connection(),
            target.sender_name.as_deref(),
            target.sender_pid,
            target.sender_start_time,
        )
        .await
        .ok_or_else(application_unavailable_error)?;

        // The test seam models concurrent replacement pressure after external liveness work
        pre_emit().await;
        let is_current = self
            .state
            .store
            .lock()
            .await
            .is_active_notification_generation(notification.id, &target);
        if !is_current {
            return Err(zbus::fdo::Error::InvalidArgs(
                "notification changed before its action could be invoked".to_string(),
            ));
        }

        // Scope the signal to the stored owner so unrelated bus listeners cannot observe it
        let context = SignalContext::new(self.state.connection(), NOTIFICATIONS_OBJECT_PATH)
            .map_err(to_fdo_error)?
            .set_destination(bus_name.to_owned());
        NotificationServer::action_invoked(&context, notification.id, action_key)
            .await
            .map_err(to_fdo_error)?;

        // A successful action consumes an ordinary notification after delivery
        if !target.is_resident {
            self.state
                .dismiss_actioned_if_current(notification.id, &target)
                .await
                .map_err(to_fdo_error)?;
        }
        Ok(())
    }
}

fn application_unavailable_error() -> zbus::fdo::Error {
    zbus::fdo::Error::Failed("The application is no longer available".to_string())
}
