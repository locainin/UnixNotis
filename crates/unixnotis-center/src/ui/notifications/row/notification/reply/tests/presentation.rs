//! Sender-provided reply presentation tests

use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::Action;

use crate::ui::icons::IconResolver;
use crate::ui::notifications::test_support::init_gtk;

use super::{
    build_notification_row, row_data, sample_notification, update_notification_row, RowFlags,
};

#[gtk::test]
fn inline_reply_submit_label_is_bounded_without_splitting_unicode() {
    init_gtk();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(4);
    let (_root, row) = build_notification_row(command_tx.clone());
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

#[gtk::test]
fn inline_reply_submit_icon_is_rendered_before_the_label() {
    init_gtk();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(4);
    let (_root, row) = build_notification_row(command_tx.clone());
    let mut notification = sample_notification();
    notification.actions = vec![Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    }];
    notification.inline_reply.available = true;
    notification.inline_reply.submit_icon = "mail-send-symbolic".to_string();

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
    assert!(content
        .first_child()
        .is_some_and(|child| child.is::<gtk::Image>()));
    assert!(content
        .last_child()
        .is_some_and(|child| child.is::<gtk::Label>()));
}
