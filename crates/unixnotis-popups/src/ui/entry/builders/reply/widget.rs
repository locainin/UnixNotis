//! GTK construction and input wiring for one popup reply editor

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::NotificationView;

use super::lifecycle::{
    bounded_reply_text, cancel_reply, submit_reply, ReplySubmission, MAX_REPLY_CHARS,
};
use crate::dbus::UiCommand;
use crate::ui::entry::presentation::{PopupEntryViewModel, PopupKind, ReplyPresentation};

pub(in crate::ui::entry) fn build_inline_reply(
    notification: &NotificationView,
    view: &PopupEntryViewModel,
    command_tx: &tokio::sync::mpsc::Sender<UiCommand>,
) -> Option<gtk::Box> {
    if view.kind != PopupKind::Communication || view.trust.reply != ReplyPresentation::Available {
        return None;
    }

    let root = gtk::Box::new(gtk::Orientation::Vertical, 4);
    root.add_css_class("unixnotis-popup-inline-reply");
    let reveal = gtk::Button::with_label(reply_label(notification));
    reveal.add_css_class("unixnotis-popup-action");
    root.append(&reveal);

    let revealer = gtk::Revealer::new();
    revealer.set_reveal_child(false);
    revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
    revealer.set_transition_duration(200);
    let form = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let input_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let entry = gtk::Entry::new();
    entry.set_hexpand(true);
    entry.set_max_length(MAX_REPLY_CHARS);
    entry.set_placeholder_text(Some(reply_placeholder(notification)));
    entry.add_css_class("unixnotis-popup-reply-entry");
    let send = gtk::Button::with_label(reply_submit_label(notification));
    send.set_sensitive(false);
    send.add_css_class("unixnotis-popup-action");
    let cancel = gtk::Button::with_label("Cancel");
    cancel.add_css_class("unixnotis-popup-action");
    let error = gtk::Label::new(None);
    error.set_xalign(0.0);
    error.set_wrap(true);
    error.set_visible(false);
    error.add_css_class("unixnotis-popup-reply-error");

    input_row.append(&entry);
    input_row.append(&send);
    input_row.append(&cancel);
    form.append(&input_row);
    form.append(&error);
    revealer.set_child(Some(&form));
    root.append(&revealer);

    let submitted = Rc::new(Cell::new(false));
    connect_reveal(&reveal, &revealer, &entry, &submitted);
    connect_validation(&entry, &send, &error, &submitted);
    connect_submission(
        notification,
        &entry,
        &revealer,
        &send,
        &error,
        &submitted,
        command_tx,
    );
    connect_cancel(&entry, &revealer, &cancel, &error, &submitted);

    Some(root)
}

fn connect_reveal(
    button: &gtk::Button,
    revealer: &gtk::Revealer,
    entry: &gtk::Entry,
    submitted: &Rc<Cell<bool>>,
) {
    let revealer = revealer.clone();
    let entry = entry.clone();
    let submitted = Rc::clone(submitted);
    button.connect_clicked(move |_| {
        // Opening the editor is local-only and never sends an application signal
        if submitted.get() {
            return;
        }
        revealer.set_reveal_child(true);
        entry.grab_focus();
    });
}

fn connect_validation(
    entry: &gtk::Entry,
    send: &gtk::Button,
    error: &gtk::Label,
    submitted: &Rc<Cell<bool>>,
) {
    let send = send.clone();
    let error = error.clone();
    let submitted = Rc::clone(submitted);
    entry.connect_changed(move |entry| {
        error.set_visible(false);
        let valid = bounded_reply_text(&entry.text()).is_some();
        send.set_sensitive(valid && !submitted.get());
        entry.set_tooltip_text(
            (!valid && !entry.text().trim().is_empty())
                .then_some("Reply text must be one line and no larger than 4 KiB"),
        );
    });
}

fn connect_submission(
    notification: &NotificationView,
    entry: &gtk::Entry,
    revealer: &gtk::Revealer,
    send: &gtk::Button,
    error: &gtk::Label,
    submitted: &Rc<Cell<bool>>,
    command_tx: &tokio::sync::mpsc::Sender<UiCommand>,
) {
    let click_entry = entry.clone();
    let click_revealer = revealer.clone();
    let click_send = send.clone();
    let click_error = error.clone();
    let click_submitted = Rc::clone(submitted);
    let click_tx = command_tx.clone();
    let id = notification.id;
    let generation = notification.generation;
    send.connect_clicked(move |_| {
        submit_reply(ReplySubmission {
            id,
            generation,
            entry: &click_entry,
            revealer: &click_revealer,
            send: &click_send,
            error: &click_error,
            submitted: &click_submitted,
            command_tx: &click_tx,
        });
    });

    let activate_revealer = revealer.clone();
    let activate_send = send.clone();
    let activate_error = error.clone();
    let activate_submitted = Rc::clone(submitted);
    let activate_tx = command_tx.clone();
    entry.connect_activate(move |entry| {
        submit_reply(ReplySubmission {
            id,
            generation,
            entry,
            revealer: &activate_revealer,
            send: &activate_send,
            error: &activate_error,
            submitted: &activate_submitted,
            command_tx: &activate_tx,
        });
    });
}

fn connect_cancel(
    entry: &gtk::Entry,
    revealer: &gtk::Revealer,
    cancel: &gtk::Button,
    error: &gtk::Label,
    submitted: &Rc<Cell<bool>>,
) {
    let cancel_entry = entry.clone();
    let cancel_revealer = revealer.clone();
    let cancel_error = error.clone();
    let cancel_submitted = Rc::clone(submitted);
    cancel.connect_clicked(move |_| {
        cancel_reply(
            &cancel_entry,
            &cancel_revealer,
            &cancel_error,
            &cancel_submitted,
        );
    });

    let key_revealer = revealer.clone();
    let key_error = error.clone();
    let key_submitted = Rc::clone(submitted);
    let controller = gtk::EventControllerKey::new();
    controller.connect_key_pressed(move |controller, key, _, _| {
        if key != gtk::gdk::Key::Escape {
            return gtk::glib::Propagation::Proceed;
        }
        if let Some(entry) = controller.widget().and_downcast::<gtk::Entry>() {
            cancel_reply(&entry, &key_revealer, &key_error, &key_submitted);
        }
        gtk::glib::Propagation::Stop
    });
    entry.add_controller(controller);
}

fn reply_label(notification: &NotificationView) -> &str {
    if notification.inline_reply.label.trim().is_empty() {
        "Reply"
    } else {
        &notification.inline_reply.label
    }
}

fn reply_placeholder(notification: &NotificationView) -> &str {
    if notification.inline_reply.placeholder.trim().is_empty() {
        "Write a reply"
    } else {
        &notification.inline_reply.placeholder
    }
}

fn reply_submit_label(notification: &NotificationView) -> &str {
    if notification.inline_reply.submit_label.trim().is_empty() {
        "Send"
    } else {
        &notification.inline_reply.submit_label
    }
}
