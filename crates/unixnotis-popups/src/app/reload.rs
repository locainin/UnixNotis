//! Reload coalescing for popup config and CSS watchers

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use glib::ControlFlow;

use crate::dbus;

const RELOAD_FLUSH_INTERVAL_MS: u64 = 200;

// No reload event is represented right now
const RELOAD_IDLE: u8 = 0;
// Channel capacity blocked a reload send, so the timer must retry it
const RELOAD_PENDING_RETRY: u8 = 1;
// A reload event is queued or currently being handled on the main loop
const RELOAD_QUEUED_OR_RUNNING: u8 = 2;

// Coalesces reload requests so config and CSS edits stay eventually consistent
// without letting bursty watcher traffic fill the queue with duplicate reloads
pub(super) struct ReloadGate {
    css: ReloadSlot,
    config: ReloadSlot,
}

// Each reload kind tracks the represented reload plus whether another watcher
// hit landed after that represented reload was already claimed
struct ReloadSlot {
    state: AtomicU8,
    dirty_again: AtomicBool,
}

impl ReloadSlot {
    fn new() -> Self {
        Self {
            state: AtomicU8::new(RELOAD_IDLE),
            dirty_again: AtomicBool::new(false),
        }
    }

    fn has_retry_pending(&self) -> bool {
        self.state.load(Ordering::Acquire) == RELOAD_PENDING_RETRY
    }

    fn request(&self, sender: &async_channel::Sender<dbus::UiEvent>, event: dbus::UiEvent) -> bool {
        loop {
            match self.state.load(Ordering::Acquire) {
                RELOAD_IDLE => {
                    // Claim the slot first so only one represented reload exists
                    // for this event kind at a time
                    if self
                        .state
                        .compare_exchange(
                            RELOAD_IDLE,
                            RELOAD_QUEUED_OR_RUNNING,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_err()
                    {
                        continue;
                    }
                    return self.dispatch(sender, event);
                }
                RELOAD_PENDING_RETRY | RELOAD_QUEUED_OR_RUNNING => {
                    // Another watcher hit landed after the represented reload
                    self.dirty_again.store(true, Ordering::Release);
                    return false;
                }
                _ => unreachable!("invalid reload slot state"),
            }
        }
    }

    fn flush(&self, sender: &async_channel::Sender<dbus::UiEvent>, event: dbus::UiEvent) {
        if self.state.load(Ordering::Acquire) != RELOAD_PENDING_RETRY {
            return;
        }

        // A retry that finally enters the queue already covers everything seen
        // up to the point where this send succeeds
        let had_trailing_change = self.dirty_again.swap(false, Ordering::AcqRel);
        match sender.try_send(event) {
            Ok(()) => {
                self.state
                    .store(RELOAD_QUEUED_OR_RUNNING, Ordering::Release);
            }
            Err(async_channel::TrySendError::Full(_)) => {
                if had_trailing_change {
                    self.dirty_again.store(true, Ordering::Release);
                }
            }
            Err(async_channel::TrySendError::Closed(_)) => {
                self.clear();
            }
        }
    }

    fn complete(
        &self,
        sender: &async_channel::Sender<dbus::UiEvent>,
        event: dbus::UiEvent,
    ) -> bool {
        let had_trailing_change = self.dirty_again.swap(false, Ordering::AcqRel);
        if had_trailing_change {
            // Another watcher hit landed while the current reload was in flight
            return self.dispatch(sender, event);
        }

        // Clear the represented slot, then recheck once more so a watcher hit
        // landing in this narrow window still becomes another reload
        self.state.store(RELOAD_IDLE, Ordering::Release);
        if self.dirty_again.swap(false, Ordering::AcqRel) {
            return self.request(sender, event);
        }
        false
    }

    fn dispatch(
        &self,
        sender: &async_channel::Sender<dbus::UiEvent>,
        event: dbus::UiEvent,
    ) -> bool {
        match sender.try_send(event) {
            Ok(()) => {
                self.state
                    .store(RELOAD_QUEUED_OR_RUNNING, Ordering::Release);
                false
            }
            Err(async_channel::TrySendError::Full(_)) => {
                self.state.store(RELOAD_PENDING_RETRY, Ordering::Release);
                true
            }
            Err(async_channel::TrySendError::Closed(_)) => {
                self.clear();
                false
            }
        }
    }

    fn clear(&self) {
        self.state.store(RELOAD_IDLE, Ordering::Release);
        self.dirty_again.store(false, Ordering::Release);
    }
}

impl ReloadGate {
    pub(super) fn new() -> Self {
        Self {
            css: ReloadSlot::new(),
            config: ReloadSlot::new(),
        }
    }

    pub(super) fn request_css(&self, sender: &async_channel::Sender<dbus::UiEvent>) -> bool {
        self.css.request(sender, dbus::UiEvent::CssReload)
    }

    pub(super) fn request_config(&self, sender: &async_channel::Sender<dbus::UiEvent>) -> bool {
        self.config.request(sender, dbus::UiEvent::ConfigReload)
    }

    pub(super) fn flush(&self, sender: &async_channel::Sender<dbus::UiEvent>) {
        self.css.flush(sender, dbus::UiEvent::CssReload);
        self.config.flush(sender, dbus::UiEvent::ConfigReload);
    }

    // This only reports retries that are still blocked on queue capacity
    // A queued or running reload is completed through complete_* instead
    pub(super) fn has_pending(&self) -> bool {
        self.css.has_retry_pending() || self.config.has_retry_pending()
    }

    pub(super) fn complete_css(&self, sender: &async_channel::Sender<dbus::UiEvent>) -> bool {
        self.css.complete(sender, dbus::UiEvent::CssReload)
    }

    pub(super) fn complete_config(&self, sender: &async_channel::Sender<dbus::UiEvent>) -> bool {
        self.config.complete(sender, dbus::UiEvent::ConfigReload)
    }
}

// Schedules one GTK timer that keeps retrying reload sends while the bounded
// queue is full
pub(super) fn start_reload_timer(
    reload_gate: &Arc<ReloadGate>,
    sender: &async_channel::Sender<dbus::UiEvent>,
    timer_state: &Arc<Mutex<Option<glib::SourceId>>>,
) {
    let mut timer_guard = match timer_state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if timer_guard.is_some() {
        return;
    }
    let reload_gate = Arc::clone(reload_gate);
    let sender = sender.clone();
    let timer_state = Arc::clone(timer_state);
    let source_id =
        glib::timeout_add_local(Duration::from_millis(RELOAD_FLUSH_INTERVAL_MS), move || {
            reload_gate.flush(&sender);
            if reload_gate.has_pending() {
                ControlFlow::Continue
            } else {
                let mut timer_guard = match timer_state.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                *timer_guard = None;
                ControlFlow::Break
            }
        });
    *timer_guard = Some(source_id);
}
