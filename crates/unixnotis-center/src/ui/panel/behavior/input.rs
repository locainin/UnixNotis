//! Small UI-side guards for bursty button and toggle input
//!
//! These helpers keep repeated clicks from spawning redundant work while still
//! letting the newest user intent win when toggles bounce quickly

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use async_channel::{Sender, TrySendError};
use gtk::glib;

use crate::control::UiEvent;

#[derive(Clone)]
pub(in crate::ui) struct ClickCooldown {
    // Block state answers whether a new click may start
    blocked: Rc<Cell<bool>>,
    // Generation keeps retired timeout callbacks from changing a newer window
    generation: Rc<Cell<u64>>,
    duration: Duration,
}

impl ClickCooldown {
    pub(in crate::ui) fn new(duration: Duration) -> Self {
        Self {
            blocked: Rc::new(Cell::new(false)),
            generation: Rc::new(Cell::new(0)),
            duration,
        }
    }

    pub(in crate::ui) fn try_start(&self) -> bool {
        if self.blocked.replace(true) {
            return false;
        }
        let ticket = self.generation.get().wrapping_add(1);
        self.generation.set(ticket);

        // GTK-side timeout keeps the guard tied to the main-thread widget lifecycle
        let blocked = self.blocked.clone();
        let generation = self.generation.clone();
        glib::timeout_add_local_once(self.duration, move || {
            release_cooldown_if_current(&blocked, &generation, ticket);
        });
        true
    }

    pub(in crate::ui) fn release(&self) {
        // Semantic actions such as Escape may end a transition immediately
        // Advancing the generation also retires the earlier timeout callback
        self.generation.set(self.generation.get().wrapping_add(1));
        self.blocked.set(false);
    }
}

fn release_cooldown_if_current(blocked: &Cell<bool>, generation: &Cell<u64>, ticket: u64) {
    // An older timeout must not release a newer cooldown window
    if generation.get() == ticket {
        blocked.set(false);
    }
}

#[derive(Clone)]
pub(in crate::ui) struct LatestBoolEventGate {
    // Stores the newest requested toggle state while one queued send is pending
    latest: Rc<Cell<bool>>,
    pending: Rc<RefCell<Option<glib::SourceId>>>,
    delay: Duration,
}

impl LatestBoolEventGate {
    pub(in crate::ui) fn new(delay: Duration) -> Self {
        Self {
            latest: Rc::new(Cell::new(false)),
            pending: Rc::new(RefCell::new(None)),
            delay,
        }
    }

    pub(in crate::ui) fn request_widgets_collapsed(
        &self,
        sender: &Sender<UiEvent>,
        collapsed: bool,
    ) {
        self.latest.set(collapsed);
        schedule_widgets_collapsed(
            sender.clone(),
            self.latest.clone(),
            self.pending.clone(),
            self.delay,
        );
    }
}

fn schedule_widgets_collapsed(
    sender: Sender<UiEvent>,
    latest: Rc<Cell<bool>>,
    pending: Rc<RefCell<Option<glib::SourceId>>>,
    delay: Duration,
) {
    // One pending source is enough because only the newest bool state matters
    if pending.borrow().is_some() {
        return;
    }

    let sender_retry = sender;
    let latest_retry = latest;
    let pending_retry = pending.clone();
    let id = glib::timeout_add_local_once(delay, move || {
        let _ = pending_retry.borrow_mut().take();
        match sender_retry.try_send(UiEvent::WidgetsCollapsed(latest_retry.get())) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                // Re-arm one more send attempt with the newest requested state
                schedule_widgets_collapsed(
                    sender_retry.clone(),
                    latest_retry.clone(),
                    pending_retry.clone(),
                    delay,
                );
            }
            Err(TrySendError::Closed(_)) => {}
        }
    });
    *pending.borrow_mut() = Some(id);
}

#[cfg(test)]
#[path = "tests/input.rs"]
mod tests;
