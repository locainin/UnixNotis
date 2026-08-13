//! Local popup display timers

use std::time::Duration;
use std::{cell::Cell, rc::Rc};

use unixnotis_core::{NotificationKey, NotificationView};

use crate::dbus::UiEvent;

use super::super::UiState;

pub(super) fn popup_display_timeout(notification: &NotificationView) -> Option<Duration> {
    // The daemon has already resolved protocol, urgency, rule, and resident policy
    let timeout_ms = notification.popup_hide_after_ms;

    // Zero disables local hiding while the active daemon record remains available
    (timeout_ms > 0).then(|| Duration::from_millis(timeout_ms))
}

impl UiState {
    pub(super) fn schedule_popup_hide(&mut self, key: NotificationKey) {
        let Some(sender) = self.popup_event_tx.clone() else {
            // Unit tests construct state without an application event channel
            return;
        };
        let Some(notification) = self
            .popups
            .get(&key.id)
            .filter(|entry| entry.notification.key() == key)
            .map(|entry| entry.notification.clone())
        else {
            return;
        };
        let Some(timeout) = popup_display_timeout(&notification) else {
            return;
        };
        let Some(entry) = self.popups.get_mut(&key.id) else {
            return;
        };
        entry.cancel_hide_timer();
        let fired = Rc::new(Cell::new(false));
        let callback_fired = Rc::clone(&fired);
        entry.hide_timer = Some(glib::timeout_add_local_once(timeout, move || {
            // This event only removes the popup process's banner
            callback_fired.set(true);
            // Wait asynchronously if the shared UI queue is briefly full
            glib::MainContext::default().spawn_local(async move {
                let _ = sender.send(UiEvent::PopupHidden(key)).await;
            });
        }));
        entry.hide_timer_fired = Some(fired);
    }
}

#[cfg(test)]
#[path = "tests/timeout.rs"]
mod tests;
