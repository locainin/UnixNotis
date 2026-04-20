//! Small UI-side guards for bursty button and toggle input.
//!
//! These helpers keep repeated clicks from spawning redundant work while still
//! letting the newest user intent win when toggles bounce quickly.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use async_channel::{Sender, TrySendError};
use gtk::glib;

use crate::dbus::UiEvent;

#[derive(Clone)]
pub(super) struct ClickCooldown {
    // One bit is enough because callers only care whether a new click may start
    blocked: Rc<Cell<bool>>,
    duration: Duration,
}

impl ClickCooldown {
    pub(super) fn new(duration: Duration) -> Self {
        Self {
            blocked: Rc::new(Cell::new(false)),
            duration,
        }
    }

    pub(super) fn try_start(&self) -> bool {
        if self.blocked.replace(true) {
            return false;
        }

        // GTK-side timeout keeps the guard tied to the main-thread widget lifecycle
        let blocked = self.blocked.clone();
        glib::timeout_add_local_once(self.duration, move || {
            blocked.set(false);
        });
        true
    }
}

#[derive(Clone)]
pub(super) struct LatestBoolEventGate {
    // Stores the newest requested toggle state while one queued send is pending
    latest: Rc<Cell<bool>>,
    pending: Rc<RefCell<Option<glib::SourceId>>>,
    delay: Duration,
}

impl LatestBoolEventGate {
    pub(super) fn new(delay: Duration) -> Self {
        Self {
            latest: Rc::new(Cell::new(false)),
            pending: Rc::new(RefCell::new(None)),
            delay,
        }
    }

    pub(super) fn request_widgets_collapsed(&self, sender: &Sender<UiEvent>, collapsed: bool) {
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

    let sender_retry = sender.clone();
    let latest_retry = latest.clone();
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
