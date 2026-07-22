//! Validation for application action signals requested by trusted control clients

use std::future::Future;

use zbus::fdo::DBusProxy;
use zbus::SignalContext;

use crate::daemon::{to_fdo_error, NotificationServer, NOTIFICATIONS_OBJECT_PATH};

use super::ControlServer;

impl ControlServer {
    pub(super) async fn invoke_validated_action(
        &self,
        id: u32,
        action_key: &str,
    ) -> zbus::fdo::Result<()> {
        self.invoke_validated_action_with_pre_emit(id, action_key, || std::future::ready(()))
            .await
    }

    pub(super) async fn invoke_validated_action_with_pre_emit<F, Fut>(
        &self,
        id: u32,
        action_key: &str,
        pre_emit: F,
    ) -> zbus::fdo::Result<()>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = ()>,
    {
        let target = {
            // Capture one concrete generation while validating the stored action identity
            let store = self.state.store.lock().await;
            store.active_action_target(id, action_key).ok_or_else(|| {
                zbus::fdo::Error::InvalidArgs(
                    "notification is not live or does not advertise this action".to_string(),
                )
            })?
        };
        let sender = target
            .sender_name
            .as_deref()
            .ok_or_else(application_unavailable_error)?;
        let bus_name = zbus::names::BusName::try_from(sender)
            .map_err(|_error| application_unavailable_error())?;
        let proxy = DBusProxy::new(self.state.connection())
            .await
            .map_err(to_fdo_error)?;
        if !proxy
            .name_has_owner(bus_name)
            .await
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?
        {
            return Err(application_unavailable_error());
        }

        // The test seam models replacement after the external liveness query
        pre_emit().await;
        let is_current = self
            .state
            .store
            .lock()
            .await
            .is_active_notification_generation(id, &target);
        if !is_current {
            return Err(zbus::fdo::Error::InvalidArgs(
                "notification changed before its action could be invoked".to_string(),
            ));
        }

        // Reuse the freedesktop signal path only after identity and liveness checks pass
        let context = SignalContext::new(self.state.connection(), NOTIFICATIONS_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        NotificationServer::action_invoked(&context, id, action_key)
            .await
            .map_err(to_fdo_error)
    }
}

fn application_unavailable_error() -> zbus::fdo::Error {
    zbus::fdo::Error::Failed("The application is no longer available".to_string())
}
