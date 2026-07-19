//! KDE-compatible inline reply handling for active notifications

use unixnotis_core::util;
use zbus::SignalContext;

use crate::daemon::{to_fdo_error, NotificationServer, NOTIFICATIONS_OBJECT_PATH};

use super::ControlServer;

pub(super) const MAX_REPLY_TEXT_BYTES: usize = 4 * 1024;

impl ControlServer {
    pub(super) async fn submit_inline_reply(
        &self,
        id: u32,
        reply_text: &str,
    ) -> zbus::fdo::Result<()> {
        // Text validation happens before any notification lookup or signal work
        let reply_text = sanitize_reply_text(reply_text)?;
        let is_resident = {
            // Keep the store lock only for the live-action eligibility snapshot
            let store = self.state.store.lock().await;
            store.active_inline_reply_target(id).ok_or_else(|| {
                zbus::fdo::Error::InvalidArgs(
                    "notification is not live or does not support inline reply".to_string(),
                )
            })?
        };

        // Emit only after all live-state and text checks have passed
        let context = SignalContext::new(self.state.connection(), NOTIFICATIONS_OBJECT_PATH)
            .map_err(to_fdo_error)?;
        NotificationServer::notification_replied(&context, id, &reply_text)
            .await
            .map_err(to_fdo_error)?;

        if !is_resident {
            // Non-resident replies leave no stale action behind in active or history lists
            self.state
                .dismiss_from_panel(id)
                .await
                .map_err(to_fdo_error)?;
        }
        // Resident notifications remain active for later updates from the sender
        Ok(())
    }
}

pub(super) fn sanitize_reply_text(reply_text: &str) -> zbus::fdo::Result<String> {
    // Display controls and line breaks are removed because GtkEntry is single-line
    let reply_text = util::sanitize_inline_display_text(reply_text);
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
    Ok(reply_text.to_string())
}
