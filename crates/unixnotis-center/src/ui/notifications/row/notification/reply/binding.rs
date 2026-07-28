//! Notification binding and action-button behavior for inline replies

use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::{InlineReplyPolicy, NotificationView};

use super::lifecycle::invalidate_reply_attempt;
use super::presentation::{clear_reply_error, update_submit_content, DEFAULT_PLACEHOLDER};
use super::state::InlineReplyWidgets;

pub(in super::super) fn configure_inline_reply(
    widgets: &InlineReplyWidgets,
    notification: &Rc<NotificationView>,
    is_active: bool,
) {
    let id = notification.id;
    let reply = &notification.inline_reply;
    // History rows keep metadata for display but never expose a live reply control
    let available = is_active
        && reply.available
        && notification.inline_reply_policy == InlineReplyPolicy::Allow;
    let snapshot_changed = widgets
        .bound_snapshot
        .borrow()
        .upgrade()
        .is_none_or(|bound| !Rc::ptr_eq(&bound, notification));
    if snapshot_changed || !available {
        // Recycled rows, replacements, and unavailable actions begin with fresh form state
        invalidate_reply_attempt(&widgets.state);
        reset_reply_form(widgets);
    }
    if snapshot_changed {
        *widgets.bound_snapshot.borrow_mut() = Rc::downgrade(notification);
    }
    // Unavailable policies also clear the command target used by click handlers
    widgets.state.bound_id.set(if available { id } else { 0 });
    widgets.state.bound_generation.set(if available {
        notification.generation
    } else {
        0
    });
    if !available {
        // History and ordinary actions never expose a stale reply field
        return;
    }

    // KDE hints customize only presentation and never change reply eligibility
    let placeholder = if reply.placeholder.is_empty() {
        DEFAULT_PLACEHOLDER
    } else {
        &reply.placeholder
    };
    widgets.entry.set_placeholder_text(Some(placeholder));
    update_submit_content(
        &widgets.send_button,
        &reply.submit_label,
        &reply.submit_icon,
    );
}

fn reset_reply_form(widgets: &InlineReplyWidgets) {
    // Every invalidation clears local-only state before the row can be reused
    widgets.entry.set_sensitive(true);
    widgets.entry.set_text("");
    widgets.send_button.set_sensitive(false);
    clear_reply_error(&widgets.error_label);
    widgets.revealer.set_reveal_child(false);
}

pub(in super::super) fn connect_inline_reply_button(
    button: &gtk::Button,
    widgets: &InlineReplyWidgets,
) {
    let revealer = widgets.revealer.clone();
    let entry = widgets.entry.clone();
    let state = widgets.state.clone();
    button.connect_clicked(move |_| {
        // Zero is the unbound sentinel and in-flight work cannot reopen the form
        if state.bound_id.get() == 0 || state.submitted.get() {
            return;
        }
        revealer.set_reveal_child(true);
        entry.grab_focus();
    });
}
