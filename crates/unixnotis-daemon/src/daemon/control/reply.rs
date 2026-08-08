//! KDE-compatible inline reply handling for active notifications

use std::future::Future;

use unixnotis_core::Notification;
use zbus::names::BusName;
use zbus::SignalContext;

use crate::daemon::notifications::identity::resolve_callback_destination;
use crate::daemon::{to_fdo_error, NotificationServer, NOTIFICATIONS_OBJECT_PATH};

use super::ControlServer;

pub(super) const MAX_REPLY_TEXT_BYTES: usize = 4 * 1024;
const APPLICATION_UNAVAILABLE: &str = "The application is no longer available";

impl ControlServer {
    pub(super) async fn submit_inline_reply(
        &self,
        id: u32,
        generation: u64,
        reply_text: &str,
    ) -> zbus::fdo::Result<()> {
        self.submit_inline_reply_with_post_emit(id, generation, reply_text, || {
            std::future::ready(())
        })
        .await
    }

    async fn submit_inline_reply_with_post_emit<F, Fut>(
        &self,
        id: u32,
        generation: u64,
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
            store
                .active_inline_reply_target(id, generation)
                .ok_or_else(|| {
                    zbus::fdo::Error::InvalidArgs(
                        "notification generation is stale or does not support inline reply"
                            .to_string(),
                    )
                })?
        };
        let destination = self.reply_destination(&target).await?;

        // A destination header keeps sensitive reply text visible only to its owning connection
        let context = SignalContext::new(self.state.connection(), NOTIFICATIONS_OBJECT_PATH)
            .map_err(to_fdo_error)?
            .set_destination(destination);
        NotificationServer::notification_replied(&context, id, reply_text)
            .await
            .map_err(to_fdo_error)?;
        // The test seam models an application replacing the row while handling the signal
        post_emit().await;

        if !target.is_resident {
            // Cleanup applies only if the exact replied generation is still stored
            self.state
                .dismiss_replied_if_current(id, &target)
                .await
                .map_err(to_fdo_error)?;
        }
        // Resident notifications remain active for later updates from the sender
        Ok(())
    }

    async fn reply_destination(
        &self,
        target: &Notification,
    ) -> zbus::fdo::Result<BusName<'static>> {
        resolve_callback_destination(
            &self.state.sender_metadata_cache,
            self.state.connection(),
            target.sender_name.as_deref(),
            target.sender_pid,
            target.sender_start_time,
        )
        .await
        .ok_or_else(application_unavailable_error)
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
