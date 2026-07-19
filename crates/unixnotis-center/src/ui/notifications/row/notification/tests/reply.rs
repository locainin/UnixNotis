use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::{Action, InlineReply};

use super::reply::{build_inline_reply, cancel_inline_reply, configure_inline_reply};
use super::test_support::{row_data, sample_notification, RowFlags};
use super::update::update_notification_row;
use crate::control::UiCommand;
use crate::ui::icons::IconResolver;
use crate::ui::notifications::test_support::init_gtk;
use crate::ui::panel::behavior::keyboard::editable_has_focus;

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
    assert!(!row.inline_reply.error_label.is_visible());
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

    let context = gtk::glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }
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
    let reply = InlineReply {
        available: true,
        ..InlineReply::default()
    };
    configure_inline_reply(&widgets, 41, &reply, true);
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
fn inline_reply_does_not_submit_before_binding_a_notification() {
    init_gtk();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);
    let widgets = build_inline_reply(command_tx);

    widgets.entry.set_text("Not bound");
    widgets.entry.emit_activate();

    assert!(command_rx.try_recv().is_err());
}

#[gtk::test]
fn inline_reply_escape_clears_an_idle_draft_and_collapses_the_form() {
    init_gtk();
    let entry = gtk::Entry::new();
    let revealer = gtk::Revealer::new();
    let error_label = gtk::Label::new(Some("Could not send"));
    let submitted = Cell::new(false);
    entry.set_text("Unsent draft");
    revealer.set_reveal_child(true);
    error_label.set_visible(true);

    assert_eq!(
        cancel_inline_reply(&entry, &revealer, &error_label, &submitted),
        gtk::glib::Propagation::Stop
    );
    assert!(entry.text().is_empty());
    assert!(!revealer.reveals_child());
    assert!(error_label.text().is_empty());
    assert!(!error_label.is_visible());
}

#[gtk::test]
fn inline_reply_key_controller_cancels_only_escape() {
    init_gtk();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(4);
    let widgets = build_inline_reply(command_tx);
    widgets.entry.set_text("Unsent draft");
    widgets.revealer.set_reveal_child(true);
    let controllers = widgets.entry.observe_controllers();
    let controller = (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .find_map(|object| object.downcast::<gtk::EventControllerKey>().ok())
        .expect("inline reply key controller");

    let proceed = controller.emit_by_name::<bool>(
        "key-pressed",
        &[&gtk::gdk::Key::a, &0_u32, &gtk::gdk::ModifierType::empty()],
    );
    assert!(!proceed);
    assert_eq!(widgets.entry.text(), "Unsent draft");

    let stop = controller.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk::gdk::Key::Escape,
            &0_u32,
            &gtk::gdk::ModifierType::empty(),
        ],
    );
    assert!(stop);
    assert!(widgets.entry.text().is_empty());
    assert!(!widgets.revealer.reveals_child());
}

#[gtk::test]
fn inline_reply_entry_focus_is_recognized_as_editable_panel_input() {
    init_gtk();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(4);
    let (root, row) = super::build::build_notification_row(command_tx);
    let window = gtk::Window::new();
    window.set_child(Some(&root));
    window.set_visible(true);

    row.inline_reply.entry.grab_focus();

    assert!(editable_has_focus(&window));
}

#[gtk::test]
fn inline_reply_dead_sender_error_uses_the_stable_user_message() {
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
    let context = gtk::glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }

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
    configure_inline_reply(&widgets, 41, &reply, true);
    widgets.entry.set_text("Old draft");
    widgets.send_button.emit_clicked();
    let _pending_reply = command_rx.try_recv().expect("pending reply command");
    assert!(!widgets.entry.is_sensitive());
    widgets.error_label.set_text("Could not send: old error");
    widgets.error_label.set_visible(true);
    widgets.revealer.set_reveal_child(true);

    configure_inline_reply(&widgets, 42, &reply, true);

    assert!(widgets.entry.text().is_empty());
    assert!(widgets.error_label.text().is_empty());
    assert!(!widgets.error_label.is_visible());
    assert!(!widgets.revealer.reveals_child());
    assert!(widgets.entry.is_sensitive());
    assert!(!widgets.send_button.is_sensitive());
}

#[gtk::test]
fn stale_reply_result_cannot_change_a_new_inflight_reply() {
    init_gtk();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);
    let widgets = build_inline_reply(command_tx);
    let reply = InlineReply {
        available: true,
        ..InlineReply::default()
    };
    configure_inline_reply(&widgets, 41, &reply, true);
    widgets.entry.set_text("First");
    widgets.entry.emit_activate();
    let UiCommand::Reply {
        outcome: first_outcome,
        ..
    } = command_rx.try_recv().expect("first reply command")
    else {
        panic!("expected inline reply command");
    };

    configure_inline_reply(&widgets, 42, &reply, true);
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

fn drain_main_context() {
    let context = gtk::glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }
}
