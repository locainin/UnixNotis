//! Authenticated notification pulls after lightweight signal delivery

use tracing::warn;
use unixnotis_core::{timed_dbus_call, ControlProxy, PopupCandidate};

use crate::dbus::UiEvent;

pub(super) async fn push_active_notification_event(
    proxy: &ControlProxy<'_>,
    sender: &async_channel::Sender<UiEvent>,
    id: u32,
    generation: u64,
    is_add: bool,
) {
    // Payload and popup policy come from one daemon-side store snapshot
    match timed_dbus_call(proxy.get_popup_candidate(id)).await {
        Ok(candidates) => {
            let Some(event) = popup_event(candidates, generation, is_add) else {
                return;
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

pub(super) fn popup_event(
    mut candidates: Vec<PopupCandidate>,
    generation: u64,
    is_add: bool,
) -> Option<UiEvent> {
    // A close signal may win this fetch race, making an empty result normal
    let candidate = candidates.pop()?;
    // A delayed signal must never lend its admission to a replacement payload
    if candidate.notification.generation != generation {
        return None;
    }
    if is_add {
        Some(UiEvent::NotificationAdded(
            candidate.notification,
            candidate.should_show,
        ))
    } else {
        Some(UiEvent::NotificationUpdated(
            candidate.notification,
            candidate.should_show,
        ))
    }
}
