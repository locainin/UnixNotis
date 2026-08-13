//! Reply submission guards shared by button and keyboard activation

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;

use crate::dbus::UiCommand;
use crate::ui::entry::commands::try_send_command;

pub(super) const MAX_REPLY_BYTES: usize = 4 * 1024;
pub(super) const MAX_REPLY_CHARS: i32 = 4 * 1024;

pub(super) struct ReplySubmission<'widget> {
    pub(super) id: u32,
    pub(super) generation: u64,
    pub(super) entry: &'widget gtk::Entry,
    pub(super) revealer: &'widget gtk::Revealer,
    pub(super) send: &'widget gtk::Button,
    pub(super) error: &'widget gtk::Label,
    pub(super) submitted: &'widget Rc<Cell<bool>>,
    pub(super) command_tx: &'widget tokio::sync::mpsc::Sender<UiCommand>,
}

pub(super) fn submit_reply(submission: ReplySubmission<'_>) {
    let Some(text) = bounded_reply_text(&submission.entry.text()) else {
        return;
    };
    // One shared cell closes the near-simultaneous Enter and click race
    if submission.submitted.replace(true) {
        return;
    }

    submission.entry.set_sensitive(false);
    submission.send.set_sensitive(false);
    submission.error.set_visible(false);
    let (outcome, result) = tokio::sync::oneshot::channel();
    try_send_command(
        submission.command_tx,
        UiCommand::Reply {
            id: submission.id,
            generation: submission.generation,
            text,
            outcome,
        },
    );

    let entry = submission.entry.clone();
    let revealer = submission.revealer.clone();
    let send = submission.send.clone();
    let error = submission.error.clone();
    let submitted = Rc::clone(submission.submitted);
    gtk::glib::MainContext::default().spawn_local(async move {
        let result = result.await;
        // Keep both activation paths locked until the daemon returns the final result
        submitted.set(false);
        entry.set_sensitive(true);
        match result {
            Ok(Ok(())) => {
                // Successful delivery clears local text and returns to the compact card
                entry.set_text("");
                send.set_sensitive(false);
                error.set_visible(false);
                revealer.set_reveal_child(false);
            }
            Ok(Err(message)) => {
                // A transport or daemon rejection keeps the draft available for correction
                error.set_text(&message);
                error.set_visible(true);
                send.set_sensitive(bounded_reply_text(&entry.text()).is_some());
                entry.grab_focus();
            }
            Err(_) => {
                error.set_text("Notification service did not return a reply result");
                error.set_visible(true);
                send.set_sensitive(bounded_reply_text(&entry.text()).is_some());
                entry.grab_focus();
            }
        }
    });
}

pub(super) fn bounded_reply_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value.len() <= MAX_REPLY_BYTES && !value.contains(['\0', '\r', '\n']))
        .then(|| value.to_string())
}

pub(super) fn cancel_reply(
    entry: &gtk::Entry,
    revealer: &gtk::Revealer,
    error: &gtk::Label,
    submitted: &Cell<bool>,
) {
    if submitted.get() {
        return;
    }
    entry.set_text("");
    error.set_visible(false);
    revealer.set_reveal_child(false);
}
