//! Slider refresh gate

use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone)]
pub(super) struct SliderRefreshGate {
    // True while one refresh command is already running
    in_flight: Rc<Cell<bool>>,
    // Remembers one trailing refresh request during bursts
    pending: Rc<Cell<bool>>,
}

impl SliderRefreshGate {
    pub(super) fn new() -> Self {
        Self {
            in_flight: Rc::new(Cell::new(false)),
            pending: Rc::new(Cell::new(false)),
        }
    }

    pub(super) fn begin_or_queue(&self) -> bool {
        if self.in_flight.get() {
            // One trailing refresh is enough to cover a burst of incoming requests
            self.pending.set(true);
            return false;
        }

        self.in_flight.set(true);
        true
    }

    pub(super) fn finish(&self) -> bool {
        // Return value tells the caller whether one queued refresh should run next
        self.in_flight.set(false);
        self.pending.replace(false)
    }

    pub(super) fn is_in_flight(&self) -> bool {
        // The scheduler uses this to avoid tight polling while a command is still running
        self.in_flight.get()
    }
}
