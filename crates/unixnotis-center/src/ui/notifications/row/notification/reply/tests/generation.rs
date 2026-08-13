//! Delayed reply result generation tests

use gtk::prelude::*;
use unixnotis_core::InlineReply;

use crate::control::UiCommand;
use crate::ui::notifications::test_support::init_gtk;

use super::support::{drain_main_context, reply_notification};
use super::{build_inline_reply, configure_inline_reply};

#[gtk::test]
fn stale_reply_result_cannot_change_a_new_inflight_reply() {
    init_gtk();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);
    let widgets = build_inline_reply(command_tx);
    let reply = InlineReply {
        available: true,
        ..InlineReply::default()
    };
    let first_notification = reply_notification(41, reply.clone());
    configure_inline_reply(&widgets, &first_notification, true);
    widgets.entry.set_text("First");
    widgets.entry.emit_activate();
    let UiCommand::Reply {
        outcome: first_outcome,
        ..
    } = command_rx.try_recv().expect("first reply command")
    else {
        panic!("expected inline reply command");
    };

    let second_notification = reply_notification(42, reply);
    configure_inline_reply(&widgets, &second_notification, true);
    widgets.entry.set_text("Second");
    widgets.entry.emit_activate();
    let UiCommand::Reply {
        outcome: second_outcome,
        ..
    } = command_rx.try_recv().expect("second reply command")
    else {
        panic!("expected inline reply command");
    };
    first_outcome
        .send(Err("stale failure".to_string()))
        .expect("first outcome receiver");
    drain_main_context();

    assert_eq!(widgets.entry.text(), "Second");
    assert!(!widgets.entry.is_sensitive());
    assert!(!widgets.send_button.is_sensitive());
    assert!(!widgets.error_label.is_visible());

    second_outcome
        .send(Ok(()))
        .expect("second outcome receiver");
    drain_main_context();
}

#[gtk::test]
fn stale_same_id_reply_result_cannot_change_a_new_attempt() {
    init_gtk();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);
    let widgets = build_inline_reply(command_tx);
    let available_reply = InlineReply {
        available: true,
        ..InlineReply::default()
    };
    let first_notification = reply_notification(41, available_reply.clone());
    configure_inline_reply(&widgets, &first_notification, true);
    widgets.entry.set_text("First");
    widgets.entry.emit_activate();
    let UiCommand::Reply {
        outcome: first_outcome,
        ..
    } = command_rx.try_recv().expect("first reply command")
    else {
        panic!("expected inline reply command");
    };

    // Same-ID replacements can temporarily remove and restore reply support
    let unavailable_notification = reply_notification(41, InlineReply::default());
    configure_inline_reply(&widgets, &unavailable_notification, true);
    let second_notification = reply_notification(41, available_reply);
    configure_inline_reply(&widgets, &second_notification, true);
    widgets.entry.set_text("Second");
    widgets.entry.emit_activate();
    let UiCommand::Reply {
        outcome: second_outcome,
        ..
    } = command_rx.try_recv().expect("second reply command")
    else {
        panic!("expected inline reply command");
    };

    first_outcome
        .send(Err("stale same-ID failure".to_string()))
        .expect("first outcome receiver");
    drain_main_context();

    assert_eq!(widgets.entry.text(), "Second");
    assert!(!widgets.entry.is_sensitive());
    assert!(!widgets.send_button.is_sensitive());
    assert!(!widgets.error_label.is_visible());

    second_outcome
        .send(Ok(()))
        .expect("second outcome receiver");
    drain_main_context();
    assert!(widgets.entry.text().is_empty());
    assert!(!widgets.revealer.reveals_child());
}

#[gtk::test]
fn stale_reply_result_cannot_change_an_always_available_same_id_replacement() {
    init_gtk();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);
    let widgets = build_inline_reply(command_tx);
    let reply = InlineReply {
        available: true,
        ..InlineReply::default()
    };
    let first_notification = reply_notification(41, reply.clone());
    configure_inline_reply(&widgets, &first_notification, true);
    widgets.entry.set_text("First");
    widgets.entry.emit_activate();
    let UiCommand::Reply {
        outcome: first_outcome,
        ..
    } = command_rx.try_recv().expect("first reply command")
    else {
        panic!("expected inline reply command");
    };

    // Snapshot identity distinguishes a replacement that keeps the same id
    let second_notification = reply_notification(41, reply);
    configure_inline_reply(&widgets, &second_notification, true);
    widgets.entry.set_text("Second");
    widgets.entry.emit_activate();
    let UiCommand::Reply {
        outcome: second_outcome,
        ..
    } = command_rx.try_recv().expect("second reply command")
    else {
        panic!("expected inline reply command");
    };

    first_outcome.send(Ok(())).expect("first outcome receiver");
    drain_main_context();

    assert_eq!(widgets.entry.text(), "Second");
    assert!(!widgets.entry.is_sensitive());
    assert!(!widgets.send_button.is_sensitive());

    second_outcome
        .send(Ok(()))
        .expect("second outcome receiver");
    drain_main_context();
    assert!(widgets.entry.text().is_empty());
    assert!(!widgets.revealer.reveals_child());
}
