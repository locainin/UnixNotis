//! Reply failure display and row recovery tests

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
fn inline_reply_dead_sender_error_uses_the_stable_user_message() {
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
    row.inline_reply.entry.set_text("Hello?");
    row.inline_reply.send_button.emit_clicked();
    let UiCommand::Reply { outcome, .. } = command_rx.try_recv().expect("reply command") else {
        panic!("expected inline reply command");
    };
    outcome
        .send(Err(
            "org.freedesktop.DBus.Error.Failed: The application is no longer available".to_string(),
        ))
        .expect("reply result receiver");
    drain_main_context();

    assert_eq!(
        row.inline_reply.error_label.text(),
        "Could not send: The application is no longer available"
    );
    assert!(row.inline_reply.error_label.is_visible());
}

#[gtk::test]
fn inline_reply_rebind_clears_draft_and_prior_error() {
    init_gtk();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);
    let widgets = build_inline_reply(command_tx);
    let reply = InlineReply {
        available: true,
        ..InlineReply::default()
    };
    let first_notification = reply_notification(41, reply.clone());
    configure_inline_reply(&widgets, &first_notification, true);
    widgets.entry.set_text("Old draft");
    widgets.send_button.emit_clicked();
    let _pending_reply = command_rx.try_recv().expect("pending reply command");
    assert!(!widgets.entry.is_sensitive());
    widgets.error_label.set_text("Could not send: old error");
    widgets.error_label.set_visible(true);
    widgets.revealer.set_reveal_child(true);

    let second_notification = reply_notification(42, reply);
    configure_inline_reply(&widgets, &second_notification, true);

    assert!(widgets.entry.text().is_empty());
    assert!(widgets.error_label.text().is_empty());
    assert!(!widgets.error_label.is_visible());
    assert!(!widgets.revealer.reveals_child());
    assert!(widgets.entry.is_sensitive());
    assert!(!widgets.send_button.is_sensitive());
}
