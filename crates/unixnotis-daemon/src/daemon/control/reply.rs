//! KDE-compatible inline reply handling for active notifications

use std::future::Future;

use unixnotis_core::Notification;
use zbus::fdo::DBusProxy;
use zbus::SignalContext;

use crate::daemon::{to_fdo_error, NotificationServer, NOTIFICATIONS_OBJECT_PATH};

use super::ControlServer;

pub(super) const MAX_REPLY_TEXT_BYTES: usize = 4 * 1024;
const APPLICATION_UNAVAILABLE: &str = "The application is no longer available";

impl ControlServer {
    pub(super) async fn submit_inline_reply(
        &self,
        id: u32,
        reply_text: &str,
    ) -> zbus::fdo::Result<()> {
        self.submit_inline_reply_with_post_emit(id, reply_text, || std::future::ready(()))
            .await
    }

    async fn submit_inline_reply_with_post_emit<F, Fut>(
        &self,
        id: u32,
        reply_text: &str,
        post_emit: F,
    ) -> zbus::fdo::Result<()>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = ()>,
    {
        // Text validation happens before any notification lookup or signal work
        let reply_text = validate_reply_text(reply_text)?;
        let target = {
            // Keep the Arc so later cleanup can distinguish a same-ID replacement
            let store = self.state.store.lock().await;
            store.active_inline_reply_target(id).ok_or_else(|| {
                zbus::fdo::Error::InvalidArgs(
                    "notification is not live or does not support inline reply".to_string(),
                )
            })?
        };
        self.ensure_reply_sender_is_live(&target).await?;

        // Emit only after all live-state and text checks have passed
        let context = SignalContext::new(self.state.connection(), NOTIFICATIONS_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        NotificationServer::notification_replied(&context, id, reply_text)
            .await
            .map_err(to_fdo_error)?;
        // The test seam models an application replacing the row while handling the signal
        post_emit().await;

        if !target.is_resident {
            // Cleanup applies only if the exact replied generation is still active
            self.state
                .dismiss_active_if_current(id, &target)
                .await
                .map_err(to_fdo_error)?;
        }
        // Resident notifications remain active for later updates from the sender
        Ok(())
    }

    async fn ensure_reply_sender_is_live(&self, target: &Notification) -> zbus::fdo::Result<()> {
        let sender = target
            .sender_name
            .as_deref()
            .ok_or_else(application_unavailable_error)?;
        let bus_name = zbus::names::BusName::try_from(sender).map_err(|error| {
            // Stored sender names should always be unique D-Bus names from message headers
            tracing::debug!(?error, "inline reply target has an invalid sender name");
            application_unavailable_error()
        })?;
        let proxy = DBusProxy::new(self.state.connection())
            .await
            .map_err(to_fdo_error)?;
        let has_owner = proxy
            .name_has_owner(bus_name)
            .await
            .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))?;
        if !has_owner {
            return Err(application_unavailable_error());
        }
        Ok(())
    }
}

pub(super) fn validate_reply_text(reply_text: &str) -> zbus::fdo::Result<&str> {
    // Outer spacing is not message content, while interior Unicode remains byte-for-byte intact
    let reply_text = reply_text.trim();
    if reply_text.is_empty() {
        return Err(zbus::fdo::Error::InvalidArgs(
            "reply text cannot be empty".to_string(),
        ));
    }
    if reply_text.len() > MAX_REPLY_TEXT_BYTES {
        // Byte limits match the D-Bus payload and remain stable across Unicode text
        return Err(zbus::fdo::Error::InvalidArgs(format!(
            "reply text exceeds {MAX_REPLY_TEXT_BYTES} bytes"
        )));
    }
    if reply_text.contains('\0') {
        return Err(zbus::fdo::Error::InvalidArgs(
            "reply text contains an embedded NUL".to_string(),
        ));
    }
    if reply_text.contains(['\r', '\n']) {
        return Err(zbus::fdo::Error::InvalidArgs(
            "reply text must contain one line".to_string(),
        ));
    }
    Ok(reply_text)
}

fn application_unavailable_error() -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(APPLICATION_UNAVAILABLE.to_string())
}

#[cfg(test)]
#[path = "tests/reply.rs"]
mod tests;
