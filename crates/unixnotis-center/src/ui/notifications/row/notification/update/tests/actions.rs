//! Action button update rules for notification rows

use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::{hooks, Action, InlineReply};

use crate::control::UiCommand;
use crate::ui::icons::IconResolver;

use super::super::super::test_support::{
    child_count, notification_row, row_data, sample_notification, RowFlags,
};
use super::{update_notification_row, visible_action_count};

#[gtk::test]
fn update_notification_row_rebuilds_actions_only_when_signature_changes() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.actions = vec![Action {
        key: "open".to_string(),
        label: "Open".to_string(),
    }];
    let data = row_data(
        Rc::new(notification.clone()),
        RowFlags {
            is_active: true,
            show_thumbnail: true,
            ..Default::default()
        },
    );
    let (command_tx, _rx) = tokio::sync::mpsc::channel(4);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);
    assert_eq!(child_count(&row.actions_box), 1);
    assert!(row.card.has_css_class(hooks::panel_card::HAS_ACTIONS));
    assert!(!row.card.has_css_class(hooks::panel_card::NO_ACTIONS));
    assert_eq!(
        row.action_cache.borrow().as_slice(),
        &[("open".to_string(), "Open".to_string())]
    );

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);
    assert_eq!(child_count(&row.actions_box), 1);
    let original_button = row
        .actions_box
        .first_child()
        .expect("original action button");

    notification.actions[0].label = "Open notification details now".to_string();
    let data = row_data(
        Rc::new(notification),
        RowFlags {
            is_active: true,
            show_thumbnail: true,
            ..Default::default()
        },
    );
    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert_eq!(child_count(&row.actions_box), 1);
    assert!(row.action_cache.borrow()[0]
        .1
        .starts_with("Open notification"));

    notification = sample_notification();
    notification.actions = vec![Action {
        key: "reply".to_string(),
        label: "Open notification details now".to_string(),
    }];
    let data = row_data(
        Rc::new(notification),
        RowFlags {
            is_active: true,
            show_thumbnail: true,
            ..Default::default()
        },
    );
    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert_eq!(child_count(&row.actions_box), 1);
    assert_eq!(row.action_cache.borrow()[0].0, "reply");

    // Repeating the unchanged update keeps the existing GTK action child
    let stable_button = row.actions_box.first_child().expect("stable action button");
    assert_ne!(stable_button, original_button);
    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);
    assert_eq!(
        row.actions_box.first_child().expect("reused action button"),
        stable_button
    );
}

#[gtk::test]
fn unverified_panel_row_hides_application_actions_like_the_popup() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.attribution = unixnotis_core::NotificationAttribution::unknown(
        "Claimed application",
        "unverified sender",
        "unknown:claimed".to_string(),
    );
    notification.actions = vec![Action {
        key: "default".to_string(),
        label: "Open".to_string(),
    }];
    let data = row_data(
        Rc::new(notification),
        RowFlags {
            is_active: true,
            ..Default::default()
        },
    );
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert_eq!(child_count(&row.actions_box), 0);
    assert!(row.card.has_css_class("unverified"));
}

#[gtk::test]
fn reply_action_cache_tracks_allow_and_deny_policy_transitions() {
    let (_root, row) = notification_row();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);
    let mut notification = sample_notification();
    notification.actions = vec![Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    }];
    notification.inline_reply.available = true;
    notification.inline_reply_policy = unixnotis_core::InlineReplyPolicy::Allow;

    let render = |notification: &unixnotis_core::NotificationView| {
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
    };

    render(&notification);
    assert_eq!(child_count(&row.actions_box), 1);

    notification.inline_reply_policy = unixnotis_core::InlineReplyPolicy::Deny;
    render(&notification);
    assert_eq!(child_count(&row.actions_box), 0);

    notification.inline_reply_policy = unixnotis_core::InlineReplyPolicy::Allow;
    render(&notification);
    assert_eq!(child_count(&row.actions_box), 1);
}

#[gtk::test]
fn update_notification_row_action_button_sends_command_once_per_click_window() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.actions = vec![Action {
        key: "open".to_string(),
        label: "Open".to_string(),
    }];
    let data = row_data(
        Rc::new(notification),
        RowFlags {
            is_active: true,
            ..Default::default()
        },
    );
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    let button = row
        .actions_box
        .first_child()
        .expect("action button")
        .downcast::<gtk::Button>()
        .expect("child should be action button");
    button.emit_clicked();

    match command_rx.try_recv().expect("action command") {
        UiCommand::InvokeAction { id, action_key } => {
            assert_eq!(id, 1);
            assert_eq!(action_key, "open");
        }
        command => panic!("expected action command, got {command:?}"),
    }

    button.emit_clicked();
    assert!(command_rx.try_recv().is_err());
}

#[gtk::test]
fn recycled_action_button_targets_the_new_notification_id() {
    let (_root, row) = notification_row();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);
    let mut first = sample_notification();
    first.actions = vec![Action {
        key: "open".to_string(),
        label: "Open".to_string(),
    }];
    let mut second = first.clone();
    second.id = 2;

    update_notification_row(
        &row,
        &row_data(
            Rc::new(first),
            RowFlags {
                is_active: true,
                ..Default::default()
            },
        ),
        &IconResolver::new(),
        &command_tx,
    );
    update_notification_row(
        &row,
        &row_data(
            Rc::new(second),
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
        .expect("recycled action button")
        .downcast::<gtk::Button>()
        .expect("child should be action button");
    button.emit_clicked();

    assert!(matches!(
        command_rx.try_recv(),
        Ok(UiCommand::InvokeAction { id: 2, action_key }) if action_key == "open"
    ));
}

#[gtk::test]
fn inactive_reply_action_stays_hidden_beside_a_regular_action() {
    let (_root, row) = notification_row();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(4);
    let mut notification = sample_notification();
    notification.actions = vec![
        Action {
            key: "open".to_string(),
            label: "Open".to_string(),
        },
        Action {
            key: "inline-reply".to_string(),
            label: "Reply".to_string(),
        },
    ];
    notification.inline_reply.available = true;

    update_notification_row(
        &row,
        &row_data(Rc::new(notification), RowFlags::default()),
        &IconResolver::new(),
        &command_tx,
    );

    assert_eq!(child_count(&row.actions_box), 1);
    let button = row
        .actions_box
        .first_child()
        .expect("regular action")
        .downcast::<gtk::Button>()
        .expect("action child should be a button");
    assert_eq!(button.label().as_deref(), Some("Open"));
}

#[gtk::test]
fn reply_action_label_prefers_hint_then_action_then_default() {
    let labels = [
        ("Hint reply", "Action reply", "Hint reply"),
        ("", "Action reply", "Action reply"),
        ("", "", "Reply"),
    ];

    for (hint, action, expected) in labels {
        let (_root, row) = notification_row();
        let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);
        let mut notification = sample_notification();
        notification.actions = vec![Action {
            key: "inline-reply".to_string(),
            label: action.to_string(),
        }];
        notification.inline_reply = InlineReply {
            available: true,
            label: hint.to_string(),
            ..InlineReply::default()
        };

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

        let button = row
            .actions_box
            .first_child()
            .expect("reply action")
            .downcast::<gtk::Button>()
            .expect("reply child should be a button");
        assert_eq!(button.label().as_deref(), Some(expected));
    }
}

#[test]
fn visible_action_count_requires_a_live_available_explicit_reply() {
    let mut notification = sample_notification();
    assert_eq!(visible_action_count(&notification, true), 0);

    notification.actions = vec![
        Action {
            key: "open".to_string(),
            label: "Open".to_string(),
        },
        Action {
            key: "dismiss".to_string(),
            label: "Dismiss".to_string(),
        },
    ];
    assert_eq!(visible_action_count(&notification, false), 2);

    notification.actions.push(Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    });
    assert_eq!(visible_action_count(&notification, true), 2);
    notification.inline_reply.available = true;
    assert_eq!(visible_action_count(&notification, false), 2);
    assert_eq!(visible_action_count(&notification, true), 3);
    notification.inline_reply_policy = unixnotis_core::InlineReplyPolicy::Deny;
    assert_eq!(visible_action_count(&notification, true), 2);
}
