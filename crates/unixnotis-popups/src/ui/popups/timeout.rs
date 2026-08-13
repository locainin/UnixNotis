//! Local popup display timers

use std::time::{Duration, Instant};
use std::{cell::Cell, rc::Rc};

use unixnotis_core::{NotificationKey, NotificationView};

use crate::dbus::UiEvent;

use super::super::UiState;

pub(in crate::ui) struct PopupHideTimer {
    // GLib source removal is valid only before its one-shot callback starts
    source: Option<glib::SourceId>,
    callback_fired: Option<Rc<Cell<bool>>>,
    // Monotonic time avoids wall-clock changes while a popup is visible
    deadline: Option<Instant>,
    remaining: Option<Duration>,
    paused: bool,
}

impl PopupHideTimer {
    pub(in crate::ui) const fn new() -> Self {
        Self {
            source: None,
            callback_fired: None,
            deadline: None,
            remaining: None,
            paused: false,
        }
    }
}

pub(super) fn popup_display_timeout(notification: &NotificationView) -> Option<Duration> {
    // The daemon has already resolved protocol, urgency, rule, and resident policy
    let timeout_ms = notification.popup_hide_after_ms;

    // Zero disables local hiding while the active daemon record remains available
    (timeout_ms > 0).then(|| Duration::from_millis(timeout_ms))
}

impl UiState {
    pub(super) fn schedule_popup_hide(&mut self, key: NotificationKey) {
        if self.popup_event_tx.is_none() {
            // Unit tests construct state without an application event channel
            return;
        }
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
        self.schedule_popup_hide_for(key, timeout);
    }

    fn schedule_popup_hide_for(&mut self, key: NotificationKey, timeout: Duration) {
        let Some(sender) = self.popup_event_tx.clone() else {
            return;
        };
        let Some(entry) = self
            .popups
            .get_mut(&key.id)
            .filter(|entry| entry.notification.key() == key)
        else {
            return;
        };
        entry.prepare_hide_timer(timeout, Instant::now());
        let fired = Rc::new(Cell::new(false));
        let callback_fired = Rc::clone(&fired);
        let source = glib::timeout_add_local_once(timeout, move || {
            // This event only removes the popup process's banner
            callback_fired.set(true);
            // Wait asynchronously if the shared UI queue is briefly full
            glib::MainContext::default().spawn_local(async move {
                let _ = sender.send(UiEvent::PopupHidden(key)).await;
            });
        });
        entry.install_hide_timer(source, fired);
    }

    pub(in crate::ui) fn pause_popup_hide(&mut self, key: NotificationKey) {
        // Click-through intentionally prevents all pointer interaction, including hover pause
        if !self.config.popups.pause_on_hover || self.config.popups.allow_click_through {
            return;
        }
        let Some(entry) = self
            .popups
            .get_mut(&key.id)
            .filter(|entry| entry.notification.key() == key && entry.is_materialized())
        else {
            return;
        };
        entry.pause_hide_timer(Instant::now());
    }

    pub(in crate::ui) fn resume_popup_hide(&mut self, key: NotificationKey) {
        // Resume is allowed after config changes so a disabled feature cannot strand a pause
        if self.popup_event_tx.is_none() {
            return;
        }
        let Some(timeout) = self
            .popups
            .get_mut(&key.id)
            .filter(|entry| entry.notification.key() == key)
            .and_then(super::super::entry::PopupEntry::resume_hide_timer)
        else {
            return;
        };
        self.schedule_popup_hide_for(key, timeout);
    }

    pub(in crate::ui) fn resume_ineligible_hover_pauses(&mut self) {
        if self.config.popups.pause_on_hover && !self.config.popups.allow_click_through {
            return;
        }
        let paused = self
            .popups
            .values()
            .filter(|entry| entry.hide_timer_is_paused())
            .map(|entry| entry.notification.key())
            .collect::<Vec<_>>();
        for key in paused {
            self.resume_popup_hide(key);
        }
    }
}

impl super::super::entry::PopupEntry {
    pub(in crate::ui) fn clear_hide_state(&mut self) {
        self.cancel_hide_source();
        self.hide_timer.deadline = None;
        self.hide_timer.remaining = None;
        self.hide_timer.paused = false;
    }

    pub(super) fn prepare_hide_timer(&mut self, duration: Duration, now: Instant) {
        self.clear_hide_state();
        self.hide_timer.deadline = now.checked_add(duration);
    }

    fn install_hide_timer(&mut self, source: glib::SourceId, fired: Rc<Cell<bool>>) {
        self.hide_timer.source = Some(source);
        self.hide_timer.callback_fired = Some(fired);
    }

    pub(super) fn pause_hide_timer(&mut self, now: Instant) -> bool {
        if self.hide_timer.paused || self.hide_timer_callback_fired() {
            return false;
        }
        let Some(deadline) = self.hide_timer.deadline.take() else {
            return false;
        };
        self.hide_timer.remaining = Some(deadline.saturating_duration_since(now));
        self.cancel_hide_source();
        self.hide_timer.paused = true;
        true
    }

    pub(super) const fn resume_hide_timer(&mut self) -> Option<Duration> {
        if !self.hide_timer.paused {
            return None;
        }
        self.hide_timer.paused = false;
        self.hide_timer.remaining.take()
    }

    pub(in crate::ui) const fn hide_timer_is_paused(&self) -> bool {
        self.hide_timer.paused
    }

    fn hide_timer_callback_fired(&self) -> bool {
        self.hide_timer
            .callback_fired
            .as_ref()
            .is_some_and(|state| state.get())
    }

    fn cancel_hide_source(&mut self) {
        let fired = self
            .hide_timer
            .callback_fired
            .take()
            .is_some_and(|state| state.get());
        if let Some(source) = self.hide_timer.source.take() {
            if !fired {
                source.remove();
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/timeout.rs"]
mod tests;
