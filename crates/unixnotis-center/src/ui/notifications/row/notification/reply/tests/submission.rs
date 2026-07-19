//! Reply validation, submission, and retry tests

use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::{Action, InlineReply};

use crate::control::UiCommand;
use crate::ui::icons::IconResolver;
use crate::ui::notifications::test_support::init_gtk;

use super::support::{drain_main_context, reply_notification};
use super::{
    build_inline_reply, build_notification_row, configure_inline_reply, row_data,
    sample_notification, update_notification_row, RowFlags,
};

#[gtk::test]
fn inline_reply_submit_sends_text_once_and_hides_after_success() {
    init_gtk();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);
    let (_root, row) = build_notification_row(command_tx.clone());
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
    drain_main_context();

    assert!(!row.inline_reply.revealer.reveals_child());
    assert!(row.inline_reply.entry.text().is_empty());
    assert!(!row.inline_reply.error_label.is_visible());
}

#[gtk::test]
fn inline_reply_rejects_empty_text_and_keeps_failed_draft_for_retry() {
    init_gtk();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);
    let (_root, row) = build_notification_row(command_tx.clone());
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
    row.inline_reply.entry.emit_activate();
    assert!(command_rx.try_recv().is_err());
    row.inline_reply.entry.set_text(&"🙂".repeat(1_025));
    assert!(!row.inline_reply.send_button.is_sensitive());
    row.inline_reply.entry.emit_activate();
    assert!(command_rx.try_recv().is_err());
    row.inline_reply.entry.set_text("Try again");
    assert!(row.inline_reply.send_button.is_sensitive());
    row.inline_reply.send_button.emit_clicked();
    let UiCommand::Reply { outcome, .. } = command_rx.try_recv().expect("reply command") else {
        panic!("expected inline reply command");
    };
    outcome
        .send(Err("temporary failure".to_string()))
        .expect("reply result receiver");
    drain_main_context();

    assert_eq!(row.inline_reply.entry.text(), "Try again");
    assert!(row.inline_reply.entry.is_sensitive());
    assert!(row.inline_reply.send_button.is_sensitive());
    assert!(row.inline_reply.error_label.is_visible());
    assert_eq!(
        row.inline_reply.error_label.text(),
        "Could not send: temporary failure"
    );

    row.inline_reply.entry.set_text("Try once more");
    assert!(!row.inline_reply.error_label.is_visible());
    assert!(row.inline_reply.error_label.text().is_empty());
}

#[gtk::test]
fn inline_reply_accepts_exact_byte_limit_and_blocks_changes_during_submission() {
    init_gtk();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);
    let widgets = build_inline_reply(command_tx);
    let notification = reply_notification(
        41,
        InlineReply {
            available: true,
            ..InlineReply::default()
        },
    );
    configure_inline_reply(&widgets, &notification, true);
    let exact_limit = "🙂".repeat(1_024);

    widgets.entry.set_text(&exact_limit);
    assert!(widgets.send_button.is_sensitive());
    widgets.entry.emit_activate();
    let pending = command_rx.try_recv().expect("exact-limit reply command");
    let UiCommand::Reply { text, .. } = pending else {
        panic!("expected inline reply command");
    };
    assert_eq!(text, exact_limit);

    widgets.entry.set_text("Changed while pending");
    assert!(!widgets.send_button.is_sensitive());
    widgets.entry.emit_activate();
    assert!(command_rx.try_recv().is_err());
}

#[gtk::test]
fn inline_reply_entry_accepts_the_limit_and_truncates_excess_characters() {
    init_gtk();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);
    let widgets = build_inline_reply(command_tx);
    let exact_limit = "a".repeat(4 * 1024);

    widgets.entry.set_text(&exact_limit);
    assert_eq!(widgets.entry.text().len(), exact_limit.len());

    // GTK applies the character cap before the byte-aware submission check
    let over_limit = "b".repeat((4 * 1024) + 1);
    widgets.entry.set_text(&over_limit);
    assert_eq!(widgets.entry.text().len(), exact_limit.len());
}

#[gtk::test]
fn inline_reply_does_not_submit_before_binding_a_notification() {
    init_gtk();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);
    let widgets = build_inline_reply(command_tx);

    widgets.entry.set_text("Not bound");
    widgets.entry.emit_activate();

    assert!(command_rx.try_recv().is_err());
}
