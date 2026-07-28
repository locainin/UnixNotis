//! Authenticated notification pulls after lightweight signal delivery

use tracing::warn;
use unixnotis_core::{timed_dbus_call, ControlProxy};

use crate::dbus::UiEvent;

pub(super) async fn push_active_notification_event(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<UiEvent>,
    id: u32,
    show_popup: bool,
    is_add: bool,
) {
    // The daemon remains the authority for the complete notification payload
    match timed_dbus_call(proxy.get_active_notification(id)).await {
        Ok(mut notifications) => {
            // A close signal may win this fetch race, making an empty result normal
            let Some(notification) = notifications.pop() else {
                return;
            };
            let event = if is_add {
                UiEvent::NotificationAdded(notification, show_popup)
            } else {
                UiEvent::NotificationUpdated(notification, show_popup)
            };
            let _ = sender.send(event).await;
        }
        Err(error) => {
            // One failed pull must not tear down an otherwise healthy generation
            warn!(
                ?error,
                id, "failed to fetch popup notification after signal"
            );
        }
    }
}
