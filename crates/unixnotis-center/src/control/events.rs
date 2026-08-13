//! Projection of notification signals into trusted UI payload events

use tracing::warn;
use unixnotis_core::{timed_dbus_call, ControlProxy, NotificationView};

use super::model::UiEvent;

pub(super) async fn push_active_notification_event(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<UiEvent>,
    id: u32,
    generation: u64,
    is_add: bool,
) {
    // Trusted UIs fetch current payloads through the authorized control method
    match timed_dbus_call(proxy.get_active_notification(id)).await {
        Ok(notifications) => {
            if let Some(event) = active_notification_event(notifications, generation, is_add) {
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
    generation: u64,
    is_add: bool,
) -> Option<UiEvent> {
    // A close may win the race before this follow-up payload fetch completes
    let notification = notifications.pop()?;
    if notification.generation != generation {
        // The fetched payload belongs to a newer commit than the delayed signal
        return None;
    }
    if is_add {
        Some(UiEvent::NotificationAdded(notification))
    } else {
        Some(UiEvent::NotificationUpdated(notification))
    }
}

#[cfg(test)]
#[path = "tests/events.rs"]
mod tests;
