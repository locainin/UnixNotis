//! Reusable inline reply form for live KDE-compatible notifications

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use tokio::sync::mpsc;
use unixnotis_core::InlineReply;

use crate::control::UiCommand;
use crate::ui::try_send_command;

const DEFAULT_PLACEHOLDER: &str = "Type a reply…";
const DEFAULT_SUBMIT_LABEL: &str = "Send";
// Button text stays compact even when the sender provides a long custom hint
const MAX_SUBMIT_LABEL_CHARS: usize = 20;
// GTK limits characters while the protocol boundary limits encoded bytes
const MAX_REPLY_CHARS: i32 = 4 * 1024;
const MAX_REPLY_BYTES: usize = 4 * 1024;

pub(super) struct InlineReplyWidgets {
    // The form is retained with the recycled row and revealed only on explicit action
    pub(super) revealer: gtk::Revealer,
    pub(super) entry: gtk::Entry,
    pub(super) send_button: gtk::Button,
    // Notification identity prevents a recycled row from leaking a prior draft
    bound_id: Rc<Cell<u32>>,
    // One shared gate covers button and Enter submissions
    submitted: Rc<Cell<bool>>,
}

pub(super) fn build_inline_reply(command_tx: mpsc::Sender<UiCommand>) -> InlineReplyWidgets {
    // Build the hidden form once so row updates only change state and metadata
    let revealer = gtk::Revealer::new();
    revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
    revealer.set_reveal_child(false);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.add_css_class("unixnotis-inline-reply");

    let entry = gtk::Entry::new();
    entry.set_hexpand(true);
    entry.set_max_length(MAX_REPLY_CHARS);
    entry.set_placeholder_text(Some(DEFAULT_PLACEHOLDER));
    entry.add_css_class("unixnotis-inline-reply-entry");

    let send_button = gtk::Button::with_label(DEFAULT_SUBMIT_LABEL);
    send_button.set_sensitive(false);
    send_button.add_css_class("unixnotis-notification-action");
    send_button.add_css_class("unixnotis-inline-reply-send");

    row.append(&entry);
    row.append(&send_button);
    revealer.set_child(Some(&row));

    let bound_id = Rc::new(Cell::new(0));
    let submitted = Rc::new(Cell::new(false));

    let changed_button = send_button.clone();
    let changed_submitted = submitted.clone();
    entry.connect_changed(move |entry| {
        // Sensitivity mirrors the daemon byte limit before any command is queued
        let text = entry.text();
        let text = text.trim();
        let too_long = text.len() > MAX_REPLY_BYTES;
        entry.set_tooltip_text(too_long.then_some("Reply text must be no larger than 4 KiB"));
        let valid = !text.is_empty() && !too_long;
        changed_button.set_sensitive(valid && !changed_submitted.get());
    });

    let submit_entry = entry.clone();
    let submit_revealer = revealer.clone();
    let submit_button = send_button.clone();
    let submit_id = bound_id.clone();
    let submit_gate = submitted.clone();
    let submit_tx = command_tx.clone();
    // Mouse submission shares the exact same guarded path as keyboard activation
    send_button.connect_clicked(move |_| {
        submit_reply(
            &submit_entry,
            &submit_revealer,
            &submit_button,
            &submit_id,
            &submit_gate,
            &submit_tx,
        );
    });

    let activate_revealer = revealer.clone();
    let activate_button = send_button.clone();
    let activate_id = bound_id.clone();
    let activate_gate = submitted.clone();
    // GtkEntry emits activate for Enter without needing a separate key handler
    entry.connect_activate(move |entry| {
        submit_reply(
            entry,
            &activate_revealer,
            &activate_button,
            &activate_id,
            &activate_gate,
            &command_tx,
        );
    });

    let key_revealer = revealer.clone();
    let key_entry = entry.clone();
    let key_submitted = submitted.clone();
    let key_controller = gtk::EventControllerKey::new();
    // Escape owns draft cancellation while other keys continue through GTK
    key_controller.connect_key_pressed(move |_, key, _, _| {
        if key != gtk::gdk::Key::Escape {
            return gtk::glib::Propagation::Proceed;
        }
        cancel_inline_reply(&key_entry, &key_revealer, &key_submitted)
    });
    entry.add_controller(key_controller);

    InlineReplyWidgets {
        revealer,
        entry,
        send_button,
        bound_id,
        submitted,
    }
}

pub(super) fn configure_inline_reply(
    widgets: &InlineReplyWidgets,
    id: u32,
    reply: &InlineReply,
    is_active: bool,
) {
    // History rows keep metadata for display but never expose a live reply control
    let available = is_active && reply.available;
    if widgets.bound_id.get() != id {
        // Recycled rows never carry typed drafts to another notification
        widgets.entry.set_text("");
        widgets.revealer.set_reveal_child(false);
        widgets.submitted.set(false);
        widgets.bound_id.set(id);
    }
    if !available {
        // History and ordinary actions never expose a stale reply field
        widgets.entry.set_text("");
        widgets.revealer.set_reveal_child(false);
        widgets.entry.set_sensitive(true);
        widgets.send_button.set_sensitive(false);
        widgets.submitted.set(false);
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

pub(super) fn connect_inline_reply_button(button: &gtk::Button, widgets: &InlineReplyWidgets) {
    let revealer = widgets.revealer.clone();
    let entry = widgets.entry.clone();
    let bound_id = widgets.bound_id.clone();
    let submitted = widgets.submitted.clone();
    button.connect_clicked(move |_| {
        // Zero is the unbound sentinel and in-flight work cannot reopen the form
        if bound_id.get() == 0 || submitted.get() {
            return;
        }
        revealer.set_reveal_child(true);
        entry.grab_focus();
    });
}

fn submit_reply(
    entry: &gtk::Entry,
    revealer: &gtk::Revealer,
    button: &gtk::Button,
    bound_id: &Rc<Cell<u32>>,
    submitted: &Rc<Cell<bool>>,
    command_tx: &mpsc::Sender<UiCommand>,
) {
    // Trim once so UI validation and the transmitted payload use the same content
    let text = entry.text().trim().to_string();
    let id = bound_id.get();
    // replace(true) closes the race between Enter and a near-simultaneous click
    if id == 0 || text.is_empty() || text.len() > MAX_REPLY_BYTES || submitted.replace(true) {
        return;
    }

    entry.set_sensitive(false);
    button.set_sensitive(false);
    // A one-shot response lets the GTK task restore the draft after transport failure
    let (outcome_tx, outcome_rx) = tokio::sync::oneshot::channel();
    try_send_command(
        command_tx,
        UiCommand::Reply {
            id,
            text,
            outcome: outcome_tx,
        },
    );

    let result_entry = entry.clone();
    let result_revealer = revealer.clone();
    let result_button = button.clone();
    let result_id = bound_id.clone();
    let result_submitted = submitted.clone();
    // The local main-context task is allowed to touch GTK widgets directly
    gtk::glib::MainContext::default().spawn_local(async move {
        let succeeded = matches!(outcome_rx.await, Ok(Ok(())));
        if result_id.get() != id || !result_submitted.get() {
            // A recycled row already owns different notification state
            return;
        }
        result_submitted.set(false);
        result_entry.set_sensitive(true);
        if succeeded {
            // Successful replies leave no draft behind in the reusable row
            result_entry.set_text("");
            result_revealer.set_reveal_child(false);
            result_button.set_sensitive(false);
        } else {
            // Keep the draft available for correction or retry
            result_button.set_sensitive(!result_entry.text().trim().is_empty());
            result_entry.grab_focus();
        }
    });
}

pub(super) fn cancel_inline_reply(
    entry: &gtk::Entry,
    revealer: &gtk::Revealer,
    submitted: &Cell<bool>,
) -> gtk::glib::Propagation {
    if submitted.get() {
        // An in-flight reply cannot be canceled into a second submission
        return gtk::glib::Propagation::Proceed;
    }
    // Canceling an idle draft restores the original action row
    entry.set_text("");
    revealer.set_reveal_child(false);
    gtk::glib::Propagation::Stop
}

fn update_submit_content(button: &gtk::Button, label: &str, icon_name: &str) {
    // Rebuild the tiny child box because KDE may change hints on replacement
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    if !icon_name.is_empty() {
        let icon = gtk::Image::from_icon_name(icon_name);
        content.append(&icon);
    }
    let label = if label.is_empty() {
        DEFAULT_SUBMIT_LABEL
    } else {
        label
    };
    let label = gtk::Label::new(Some(clamp_submit_label(label).as_ref()));
    content.append(&label);
    button.set_child(Some(&content));
}

fn clamp_submit_label(label: &str) -> std::borrow::Cow<'_, str> {
    // Character indexes preserve UTF-8 boundaries while enforcing visual length
    let Some((cut, _)) = label.char_indices().nth(MAX_SUBMIT_LABEL_CHARS) else {
        return std::borrow::Cow::Borrowed(label);
    };
    let mut bounded = String::with_capacity(cut + 3);
    bounded.push_str(&label[..cut]);
    bounded.push('…');
    std::borrow::Cow::Owned(bounded)
}
