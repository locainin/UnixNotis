//! Shared state for a reusable inline reply form

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use unixnotis_core::NotificationView;

use crate::ui::motion::apply_revealer_preference;

pub(super) const INLINE_REPLY_TRANSITION_MS: u32 = 250;

#[derive(Clone)]
pub(super) struct ReplyState {
    // Numeric identity is retained for the command sent to the daemon
    pub(super) bound_id: Rc<Cell<u32>>,
    // One shared gate covers button and Enter submissions
    pub(super) submitted: Rc<Cell<bool>>,
    // Attempt identity keeps delayed outcomes tied to one exact submission
    pub(super) attempt: Rc<Cell<u64>>,
}

impl ReplyState {
    pub(super) fn new() -> Self {
        Self {
            bound_id: Rc::new(Cell::new(0)),
            submitted: Rc::new(Cell::new(false)),
            attempt: Rc::new(Cell::new(0)),
        }
    }
}

pub(in super::super) struct InlineReplyWidgets {
    // The form is retained with the recycled row and revealed only on explicit action
    pub(in super::super) revealer: gtk::Revealer,
    pub(in super::super) entry: gtk::Entry,
    pub(in super::super) send_button: gtk::Button,
    pub(in super::super) error_label: gtk::Label,
    // Snapshot identity distinguishes replacements that deliberately keep the same id
    pub(super) bound_snapshot: RefCell<Weak<NotificationView>>,
    // Shared submission state keeps every GTK callback on the same generation
    pub(super) state: ReplyState,
}

impl InlineReplyWidgets {
    pub(super) const fn new(
        revealer: gtk::Revealer,
        entry: gtk::Entry,
        send_button: gtk::Button,
        error_label: gtk::Label,
        state: ReplyState,
    ) -> Self {
        Self {
            revealer,
            entry,
            send_button,
            error_label,
            bound_snapshot: RefCell::new(Weak::new()),
            state,
        }
    }

    pub(in super::super) fn set_reduced_motion(&self, reduced_motion: bool) {
        apply_revealer_preference(&self.revealer, INLINE_REPLY_TRANSITION_MS, reduced_motion);
    }
}
