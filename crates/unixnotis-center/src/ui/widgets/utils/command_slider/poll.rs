//! Command slider polling decisions

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::super::{CommandWatch, RefreshBackoff, INFLIGHT_REFRESH_RECHECK};
use super::gate::SliderRefreshGate;

pub(super) fn needs_polling(watch_handle: &RefCell<Option<CommandWatch>>) -> bool {
    let mut handle = watch_handle.borrow_mut();
    if let Some(watch) = handle.as_ref() {
        // If the watch command exited, fall back to polling and allow a new watch later
        if !watch.is_active() {
            handle.take();
            return true;
        }
        return false;
    }

    true
}

pub(super) fn next_poll_in(
    watch_handle: &RefCell<Option<CommandWatch>>,
    refresh_gate: &SliderRefreshGate,
    refresh_backoff: &Rc<RefCell<RefreshBackoff>>,
    now: Instant,
    base_interval: Duration,
) -> Option<Duration> {
    if !needs_polling(watch_handle) {
        return None;
    }

    if refresh_gate.is_in_flight() {
        // A slow command only needs a health check while its worker is still running
        return Some(INFLIGHT_REFRESH_RECHECK);
    }

    refresh_backoff.borrow().next_due_in(now).or(Some(
        Duration::ZERO.max(base_interval.min(Duration::from_millis(1))),
    ))
}
