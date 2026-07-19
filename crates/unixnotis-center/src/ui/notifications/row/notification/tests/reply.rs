use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::{Action, InlineReply};

use super::reply::cancel_inline_reply;
use super::test_support::{row_data, sample_notification, RowFlags};
use super::update::update_notification_row;
use crate::control::UiCommand;
use crate::ui::icons::IconResolver;
use crate::ui::notifications::test_support::init_gtk;

#[gtk::test]
fn inline_reply_is_available_only_for_a_live_explicit_reply_action() {
    init_gtk();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(4);
    let (_root, row) = super::build::build_notification_row(command_tx.clone());
    let mut notification = sample_notification();
    notification.actions = vec![
        Action {
            key: "inline-reply".to_string(),
            label: "Reply".to_string(),
        },
        Action {
            key: "inline-reply".to_string(),
            label: "Duplicate reply".to_string(),
        },
    ];
    notification.inline_reply = InlineReply {
        available: true,
        label: "Reply".to_string(),
        placeholder: "Write back".to_string(),
        submit_label: "Send now".to_string(),
        submit_icon: String::new(),
    };

    update_notification_row(
        &row,
        &row_data(
            Rc::new(notification.clone()),
            RowFlags {
                is_active: true,
                ..Default::default()
            },
        ),
        &IconResolver::new(),
        &command_tx,
    );
    let button = row
        .actions_box
        .first_child()
        .expect("reply action")
        .downcast::<gtk::Button>()
        .expect("reply child should be a button");
    assert!(button.next_sibling().is_none());
    button.emit_clicked();
    assert!(row.inline_reply.revealer.reveals_child());
    assert_eq!(
        row.inline_reply.entry.placeholder_text().as_deref(),
        Some("Write back")
    );

    update_notification_row(
        &row,
        &row_data(Rc::new(notification), RowFlags::default()),
        &IconResolver::new(),
        &command_tx,
    );
    assert!(!row.inline_reply.revealer.reveals_child());
    assert!(row.actions_box.first_child().is_none());
}

#[gtk::test]
fn inline_reply_submit_sends_text_once_and_hides_after_success() {
    init_gtk();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);
    let (_root, row) = super::build::build_notification_row(command_tx.clone());
    let mut notification = sample_notification();
    notification.actions = vec![Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    }];
    notification.inline_reply.available = true;

    update_notification_row(
        &row,
        &row_data(
            Rc::new(notification),
            RowFlags {
                is_active: true,
                ..Default::default()
            },
        ),
        &IconResolver::new(),
        &command_tx,
    );
    row.inline_reply.entry.set_text("On my way");
    row.inline_reply.entry.emit_activate();
    row.inline_reply.send_button.emit_clicked();

    let UiCommand::Reply { id, text, outcome } = command_rx.try_recv().expect("reply command")
    else {
        panic!("expected inline reply command");
    };
    assert_eq!(id, 1);
    assert_eq!(text, "On my way");
    assert!(command_rx.try_recv().is_err());
    outcome.send(Ok(())).expect("reply result receiver");

    let context = gtk::glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }
    assert!(!row.inline_reply.revealer.reveals_child());
    assert!(row.inline_reply.entry.text().is_empty());
}

#[gtk::test]
fn inline_reply_rejects_empty_text_and_keeps_failed_draft_for_retry() {
    init_gtk();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);
    let (_root, row) = super::build::build_notification_row(command_tx.clone());
    let mut notification = sample_notification();
    notification.actions = vec![Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    }];
    notification.inline_reply.available = true;

    update_notification_row(
        &row,
        &row_data(
            Rc::new(notification),
            RowFlags {
                is_active: true,
                ..Default::default()
            },
        ),
        &IconResolver::new(),
        &command_tx,
    );

    row.inline_reply.entry.set_text("   ");
    assert!(!row.inline_reply.send_button.is_sensitive());
    row.inline_reply.entry.set_text(&"🙂".repeat(1_025));
    assert!(!row.inline_reply.send_button.is_sensitive());
    row.inline_reply.entry.set_text("Try again");
    row.inline_reply.send_button.emit_clicked();
    let UiCommand::Reply { outcome, .. } = command_rx.try_recv().expect("reply command") else {
        panic!("expected inline reply command");
    };
    outcome
        .send(Err("temporary failure".to_string()))
        .expect("reply result receiver");

    let context = gtk::glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }
    assert_eq!(row.inline_reply.entry.text(), "Try again");
    assert!(row.inline_reply.entry.is_sensitive());
    assert!(row.inline_reply.send_button.is_sensitive());
}

#[gtk::test]
fn inline_reply_escape_clears_an_idle_draft_and_collapses_the_form() {
    init_gtk();
    let entry = gtk::Entry::new();
    let revealer = gtk::Revealer::new();
    let submitted = Cell::new(false);
    entry.set_text("Unsent draft");
    revealer.set_reveal_child(true);

    assert_eq!(
        cancel_inline_reply(&entry, &revealer, &submitted),
        gtk::glib::Propagation::Stop
    );
    assert!(entry.text().is_empty());
    assert!(!revealer.reveals_child());
}

#[gtk::test]
fn inline_reply_submit_label_is_bounded_without_splitting_unicode() {
    init_gtk();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(4);
    let (_root, row) = super::build::build_notification_row(command_tx.clone());
    let mut notification = sample_notification();
    notification.actions = vec![Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    }];
    notification.inline_reply.available = true;
    notification.inline_reply.submit_label = "界".repeat(22);

    update_notification_row(
        &row,
        &row_data(
            Rc::new(notification),
            RowFlags {
                is_active: true,
                ..Default::default()
            },
        ),
        &IconResolver::new(),
        &command_tx,
    );

    let content = row
        .inline_reply
        .send_button
        .child()
        .expect("submit content")
        .downcast::<gtk::Box>()
        .expect("submit content box");
    let label = content
        .last_child()
        .expect("submit label")
        .downcast::<gtk::Label>()
        .expect("submit label widget");
    assert_eq!(label.text(), format!("{}…", "界".repeat(20)));
}
