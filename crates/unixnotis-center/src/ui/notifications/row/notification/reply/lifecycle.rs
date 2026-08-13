//! Submission and cancellation lifecycle for inline replies

use std::cell::Cell;

use gtk::prelude::*;
use tokio::sync::mpsc;

use crate::control::UiCommand;
use crate::ui::try_send_command;

use super::presentation::{clear_reply_error, show_reply_error};
use super::state::ReplyState;

pub(super) const MAX_REPLY_BYTES: usize = 4 * 1024;

pub(super) fn submit_reply(
    entry: &gtk::Entry,
    revealer: &gtk::Revealer,
    button: &gtk::Button,
    error_label: &gtk::Label,
    state: &ReplyState,
    command_tx: &mpsc::Sender<UiCommand>,
) {
    // Trim once so UI validation and the transmitted payload use the same content
    let text = entry.text().trim().to_string();
    let id = state.bound_id.get();
    let generation = state.bound_generation.get();
    // replace(true) closes the race between Enter and a near-simultaneous click
    if id == 0
        || generation == 0
        || text.is_empty()
        || text.len() > MAX_REPLY_BYTES
        || state.submitted.replace(true)
    {
        return;
    }
    let current_attempt = state.attempt.get().wrapping_add(1);
    state.attempt.set(current_attempt);

    entry.set_sensitive(false);
    button.set_sensitive(false);
    clear_reply_error(error_label);
    // A one-shot response lets the GTK task restore the draft after transport failure
    let (outcome_tx, outcome_rx) = tokio::sync::oneshot::channel();
    try_send_command(
        command_tx,
        UiCommand::Reply {
            id,
            generation,
            text,
            outcome: outcome_tx,
        },
    );

    let result_entry = entry.clone();
    let result_revealer = revealer.clone();
    let result_button = button.clone();
    let result_error = error_label.clone();
    let result_state = state.clone();
    // The local main-context task is allowed to touch GTK widgets directly
    gtk::glib::MainContext::default().spawn_local(async move {
        let result = outcome_rx
            .await
            .unwrap_or_else(|_| Err("notification service did not return a result".to_string()));
        if result_state.bound_id.get() != id
            || result_state.bound_generation.get() != generation
            || result_state.attempt.get() != current_attempt
            || !result_state.submitted.get()
        {
            // A recycled row already owns different notification state
            return;
        }
        result_state.submitted.set(false);
        result_entry.set_sensitive(true);
        match result {
            Ok(()) => {
                // Successful replies leave no draft behind in the reusable row
                result_entry.set_text("");
                clear_reply_error(&result_error);
                result_revealer.set_reveal_child(false);
                result_button.set_sensitive(false);
            }
            Err(error) => {
                // Keep the draft available for correction or retry
                result_button.set_sensitive(!result_entry.text().trim().is_empty());
                show_reply_error(&result_error, &error);
                result_entry.grab_focus();
            }
        }
    });
}

pub(super) fn invalidate_reply_attempt(state: &ReplyState) {
    // Advancing first makes every delayed result stale before the form is reset
    state.attempt.set(state.attempt.get().wrapping_add(1));
    state.submitted.set(false);
}

pub(super) fn cancel_inline_reply(
    entry: &gtk::Entry,
    revealer: &gtk::Revealer,
    error_label: &gtk::Label,
    submitted: &Cell<bool>,
) -> gtk::glib::Propagation {
    if submitted.get() {
        // An in-flight reply cannot be canceled into a second submission
        return gtk::glib::Propagation::Proceed;
    }
    // Canceling an idle draft restores the original action row
    entry.set_text("");
    clear_reply_error(error_label);
    revealer.set_reveal_child(false);
    gtk::glib::Propagation::Stop
}
