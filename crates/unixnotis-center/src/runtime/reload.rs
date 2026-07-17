//! Lossless reload coalescing for CSS and configuration watchers

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::control::UiEvent;

const RELOAD_FLUSH_INTERVAL_MS: u64 = 200;

pub(super) struct ReloadGate {
    css: ReloadSlot,
    config: ReloadSlot,
}

struct ReloadSlot {
    state: Mutex<ReloadSlotState>,
}

#[derive(Default)]
struct ReloadSlotState {
    represented: bool,
    retry_pending: bool,
    dirty_again: bool,
}

impl ReloadSlot {
    const fn new() -> Self {
        Self {
            state: Mutex::new(ReloadSlotState {
                represented: false,
                retry_pending: false,
                dirty_again: false,
            }),
        }
    }

    fn request(&self, sender: &async_channel::Sender<UiEvent>, event: UiEvent) -> bool {
        let mut state = self.lock_state();
        let needs_retry = if state.represented {
            // Preserve one trailing reload when a change lands during processing
            state.dirty_again = true;
            false
        } else {
            state.represented = true;
            Self::dispatch(&mut state, sender, event)
        };
        drop(state);
        needs_retry
    }

    fn dispatch(
        state: &mut ReloadSlotState,
        sender: &async_channel::Sender<UiEvent>,
        event: UiEvent,
    ) -> bool {
        match sender.try_send(event) {
            Ok(()) => {
                state.retry_pending = false;
                false
            }
            Err(async_channel::TrySendError::Full(_)) => {
                state.retry_pending = true;
                true
            }
            Err(async_channel::TrySendError::Closed(_)) => {
                *state = ReloadSlotState::default();
                false
            }
        }
    }

    fn flush(&self, sender: &async_channel::Sender<UiEvent>, event: UiEvent) {
        let mut state = self.lock_state();
        if state.retry_pending {
            // A successful retry covers every change observed before it entered the queue
            let had_trailing_change = std::mem::take(&mut state.dirty_again);
            let _needs_retry = Self::dispatch(&mut state, sender, event);
            if state.retry_pending && had_trailing_change {
                state.dirty_again = true;
            }
        }
        drop(state);
    }

    fn complete(&self, sender: &async_channel::Sender<UiEvent>, event: UiEvent) -> bool {
        let mut state = self.lock_state();
        let needs_retry = if std::mem::take(&mut state.dirty_again) {
            Self::dispatch(&mut state, sender, event)
        } else {
            state.represented = false;
            false
        };
        drop(state);
        needs_retry
    }

    fn has_retry_pending(&self) -> bool {
        let state = self.lock_state();
        let retry_pending = state.retry_pending;
        drop(state);
        retry_pending
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, ReloadSlotState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

impl ReloadGate {
    pub(super) const fn new() -> Self {
        Self {
            css: ReloadSlot::new(),
            config: ReloadSlot::new(),
        }
    }

    pub(super) fn request_css(&self, sender: &async_channel::Sender<UiEvent>) -> bool {
        self.css.request(sender, UiEvent::CssReload)
    }

    pub(super) fn request_config(&self, sender: &async_channel::Sender<UiEvent>) -> bool {
        self.config.request(sender, UiEvent::ConfigReload)
    }

    pub(super) fn flush(&self, sender: &async_channel::Sender<UiEvent>) {
        self.css.flush(sender, UiEvent::CssReload);
        self.config.flush(sender, UiEvent::ConfigReload);
    }

    pub(super) fn has_pending(&self) -> bool {
        self.css.has_retry_pending() || self.config.has_retry_pending()
    }

    pub(super) fn complete_css(&self, sender: &async_channel::Sender<UiEvent>) -> bool {
        self.css.complete(sender, UiEvent::CssReload)
    }

    pub(super) fn complete_config(&self, sender: &async_channel::Sender<UiEvent>) -> bool {
        self.config.complete(sender, UiEvent::ConfigReload)
    }
}

pub(super) fn start_reload_timer(
    reload_gate: &Arc<ReloadGate>,
    sender: &async_channel::Sender<UiEvent>,
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
                glib::ControlFlow::Continue
            } else {
                let mut timer_guard = match timer_state.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                *timer_guard = None;
                glib::ControlFlow::Break
            }
        });
    *timer_guard = Some(source_id);
}

#[cfg(test)]
#[path = "tests/reload.rs"]
mod tests;
