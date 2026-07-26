//! Projection of notification signals into trusted UI payload events

use tracing::warn;
use unixnotis_core::{timed_dbus_call, ControlProxy, NotificationView};

use super::model::UiEvent;

pub(super) async fn push_active_notification_event(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<UiEvent>,
    id: u32,
    show_popup: bool,
    is_add: bool,
) {
    // Trusted UIs fetch current payloads through the authorized control method
    match timed_dbus_call(proxy.get_active_notification(id)).await {
        Ok(notifications) => {
            if let Some(event) = active_notification_event(notifications, show_popup, is_add) {
                let _ = sender.send(event).await;
            }
        }
        Err(err) => {
            warn!(?err, id, "failed to fetch active notification after signal");
        }
    }
}

fn active_notification_event(
    mut notifications: Vec<NotificationView>,
    show_popup: bool,
    is_add: bool,
) -> Option<UiEvent> {
    // A close may win the race before this follow-up payload fetch completes
    let notification = notifications.pop()?;
    if is_add {
        Some(UiEvent::NotificationAdded(notification, show_popup))
    } else {
        Some(UiEvent::NotificationUpdated(notification, show_popup))
    }
}

#[cfg(test)]
#[path = "tests/events.rs"]
mod tests;
