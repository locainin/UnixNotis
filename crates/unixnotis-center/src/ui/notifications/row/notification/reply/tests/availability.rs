//! Reply action availability and row binding tests

use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::{Action, InlineReply, InlineReplyPolicy};

use crate::ui::icons::IconResolver;
use crate::ui::notifications::test_support::init_gtk;

use super::support::reply_notification;
use super::{
    build_inline_reply, build_notification_row, configure_inline_reply,
    connect_inline_reply_button, row_data, sample_notification, update_notification_row, RowFlags,
};

#[gtk::test]
fn inline_reply_is_available_only_for_a_live_explicit_reply_action() {
    init_gtk();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(4);
    let (_root, row) = build_notification_row(command_tx.clone());
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
fn inline_reply_action_does_not_open_an_unbound_or_submitted_form() {
    init_gtk();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);
    let widgets = build_inline_reply(command_tx);
    let action = gtk::Button::new();
    connect_inline_reply_button(&action, &widgets);

    action.emit_clicked();
    assert!(!widgets.revealer.reveals_child());

    let notification = reply_notification(
        41,
        InlineReply {
            available: true,
            ..InlineReply::default()
        },
    );
    configure_inline_reply(&widgets, &notification, true);
    widgets.entry.set_text("Pending");
    widgets.entry.emit_activate();
    let _pending = command_rx.try_recv().expect("pending reply command");
    action.emit_clicked();

    assert!(!widgets.revealer.reveals_child());
}

#[gtk::test]
fn denied_inline_reply_policy_never_binds_the_form() {
    init_gtk();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);
    let widgets = build_inline_reply(command_tx);
    let mut notification = reply_notification(
        41,
        InlineReply {
            available: true,
            ..InlineReply::default()
        },
    );
    Rc::make_mut(&mut notification).inline_reply_policy = InlineReplyPolicy::Deny;

    configure_inline_reply(&widgets, &notification, true);

    assert_eq!(widgets.state.bound_id.get(), 0);
    assert!(!widgets.send_button.is_sensitive());
    assert!(!widgets.revealer.reveals_child());
}

#[gtk::test]
fn inactive_inline_reply_binding_clears_the_live_draft() {
    init_gtk();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(4);
    let widgets = build_inline_reply(command_tx);
    let notification = reply_notification(
        41,
        InlineReply {
            available: true,
            ..InlineReply::default()
        },
    );
    configure_inline_reply(&widgets, &notification, true);
    widgets.entry.set_text("Live draft");
    widgets.revealer.set_reveal_child(true);

    configure_inline_reply(&widgets, &notification, false);

    assert!(widgets.entry.text().is_empty());
    assert!(!widgets.revealer.reveals_child());
    assert!(!widgets.send_button.is_sensitive());
}
