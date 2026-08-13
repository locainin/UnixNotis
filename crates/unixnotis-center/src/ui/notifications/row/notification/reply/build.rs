//! Inline reply widget construction and input signal wiring

use gtk::prelude::*;
use tokio::sync::mpsc;

use crate::control::UiCommand;
use unixnotis_ui::presentation::default_activation::mark_interactive;

use super::lifecycle::{cancel_inline_reply, submit_reply, MAX_REPLY_BYTES};
use super::presentation::{clear_reply_error, DEFAULT_PLACEHOLDER, DEFAULT_SUBMIT_LABEL};
use super::state::{InlineReplyWidgets, ReplyState, INLINE_REPLY_TRANSITION_MS};

// GTK limits characters while the protocol boundary limits encoded bytes
const MAX_REPLY_CHARS: i32 = 4 * 1024;

pub(in super::super) fn build_inline_reply(
    command_tx: mpsc::Sender<UiCommand>,
) -> InlineReplyWidgets {
    // Build the hidden form once so row updates only change state and metadata
    let revealer = gtk::Revealer::new();
    revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
    revealer.set_transition_duration(INLINE_REPLY_TRANSITION_MS);
    revealer.set_reveal_child(false);
    mark_interactive(&revealer);

    let form = gtk::Box::new(gtk::Orientation::Vertical, 4);
    form.add_css_class("unixnotis-inline-reply");
    let input_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);

    let entry = gtk::Entry::new();
    entry.set_hexpand(true);
    entry.set_max_length(MAX_REPLY_CHARS);
    entry.set_placeholder_text(Some(DEFAULT_PLACEHOLDER));
    entry.add_css_class("unixnotis-inline-reply-entry");
    mark_interactive(&entry);

    let send_button = gtk::Button::with_label(DEFAULT_SUBMIT_LABEL);
    send_button.set_sensitive(false);
    send_button.add_css_class("unixnotis-notification-action");
    send_button.add_css_class("unixnotis-inline-reply-send");
    mark_interactive(&send_button);

    let error_label = gtk::Label::new(None);
    error_label.set_xalign(0.0);
    error_label.set_wrap(true);
    error_label.set_visible(false);
    error_label.add_css_class("error");
    error_label.add_css_class("unixnotis-inline-reply-error");

    input_row.append(&entry);
    input_row.append(&send_button);
    form.append(&input_row);
    form.append(&error_label);
    revealer.set_child(Some(&form));

    let state = ReplyState::new();
    connect_draft_changes(&entry, &send_button, &error_label, &state);
    connect_submission(
        &entry,
        &revealer,
        &send_button,
        &error_label,
        &state,
        command_tx,
    );
    connect_cancel_key(&entry, &revealer, &error_label, &state);

    InlineReplyWidgets::new(revealer, entry, send_button, error_label, state)
}

fn connect_draft_changes(
    entry: &gtk::Entry,
    send_button: &gtk::Button,
    error_label: &gtk::Label,
    state: &ReplyState,
) {
    let changed_button = send_button.clone();
    let changed_submitted = state.submitted.clone();
    let changed_error = error_label.clone();
    entry.connect_changed(move |entry| {
        // Editing clears the prior transport error because it described an older draft
        clear_reply_error(&changed_error);
        // Sensitivity mirrors the daemon byte limit before any command is queued
        let text = entry.text();
        let text = text.trim();
        let too_long = text.len() > MAX_REPLY_BYTES;
        entry.set_tooltip_text(too_long.then_some("Reply text must be no larger than 4 KiB"));
        let valid = !text.is_empty() && !too_long;
        changed_button.set_sensitive(valid && !changed_submitted.get());
    });
}

fn connect_submission(
    entry: &gtk::Entry,
    revealer: &gtk::Revealer,
    send_button: &gtk::Button,
    error_label: &gtk::Label,
    state: &ReplyState,
    command_tx: mpsc::Sender<UiCommand>,
) {
    let submit_entry = entry.clone();
    let submit_revealer = revealer.clone();
    let submit_button = send_button.clone();
    let submit_error = error_label.clone();
    let submit_state = state.clone();
    let submit_tx = command_tx.clone();
    // Mouse submission shares the exact same guarded path as keyboard activation
    send_button.connect_clicked(move |_| {
        submit_reply(
            &submit_entry,
            &submit_revealer,
            &submit_button,
            &submit_error,
            &submit_state,
            &submit_tx,
        );
    });

    let activate_revealer = revealer.clone();
    let activate_button = send_button.clone();
    let activate_error = error_label.clone();
    let activate_state = state.clone();
    // GtkEntry emits activate for Enter without needing a separate key handler
    entry.connect_activate(move |entry| {
        submit_reply(
            entry,
            &activate_revealer,
            &activate_button,
            &activate_error,
            &activate_state,
            &command_tx,
        );
    });
}

fn connect_cancel_key(
    entry: &gtk::Entry,
    revealer: &gtk::Revealer,
    error_label: &gtk::Label,
    state: &ReplyState,
) {
    let key_revealer = revealer.clone();
    let key_entry = entry.clone();
    let key_error = error_label.clone();
    let key_submitted = state.submitted.clone();
    let key_controller = gtk::EventControllerKey::new();
    // Escape owns draft cancellation while other keys continue through GTK
    key_controller.connect_key_pressed(move |_, key, _, _| {
        if key != gtk::gdk::Key::Escape {
            return gtk::glib::Propagation::Proceed;
        }
        cancel_inline_reply(&key_entry, &key_revealer, &key_error, &key_submitted)
    });
    entry.add_controller(key_controller);
}
