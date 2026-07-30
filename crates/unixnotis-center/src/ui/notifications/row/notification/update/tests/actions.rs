//! Action button update rules for notification rows

use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::{hooks, Action, ApplicationActionPolicy, InlineReply};

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
        &[(
            "open".to_string(),
            "Open".to_string(),
            ApplicationActionPolicy::Allow,
        )]
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
    notification.attribution = unixnotis_core::NotificationAttribution::unresolved(
        "Claimed application",
        unixnotis_core::AttributionReason::MissingSenderEvidence,
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
    assert!(row.card.has_css_class("unresolved"));
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
        UiCommand::InvokeAction {
            notification,
            action_key,
            confirmed,
        } => {
            assert_eq!(notification.id, 1);
            assert_eq!(notification.generation, 1);
            assert_eq!(action_key, "open");
            assert!(!confirmed, "allowed action should not claim confirmation");
        }
        command => panic!("expected action command, got {command:?}"),
    }

    button.emit_clicked();
    assert!(command_rx.try_recv().is_err());
}

#[gtk::test]
fn recycled_action_button_targets_the_new_notification_generation() {
    let (_root, row) = notification_row();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(4);
    let mut first = sample_notification();
    first.actions = vec![Action {
        key: "open".to_string(),
        label: "Open".to_string(),
    }];
    let mut second = first.clone();
    second.id = 2;
    second.generation = 7;

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
        Ok(UiCommand::InvokeAction { notification, action_key, confirmed: false })
            if notification.id == 2
                && notification.generation == 7
                && action_key == "open"
    ));
}

#[gtk::test]
fn inactive_history_row_hides_every_application_action() {
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

    assert_eq!(child_count(&row.actions_box), 0);
    assert!(row.card.has_css_class(hooks::panel_card::NO_ACTIONS));
}

#[gtk::test]
fn active_blank_default_action_builds_accessible_open_control() {
    let (_root, row) = notification_row();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(2);
    let mut notification = sample_notification();
    notification.actions = vec![Action {
        key: "default".to_string(),
        label: String::new(),
    }];

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
        .and_downcast::<gtk::Button>()
        .expect("blank default action control");
    assert!(button.has_css_class("unixnotis-panel-default-action"));
    assert_eq!(button.tooltip_text().as_deref(), Some("Open notification"));
    button.emit_clicked();
    assert!(matches!(
        command_rx.try_recv(),
        Ok(UiCommand::InvokeAction { notification, action_key, confirmed: false })
            if notification.id == 1
                && notification.generation == 1
                && action_key == "default"
    ));
}

#[gtk::test]
fn labeled_default_action_uses_one_compact_accessible_open_control() {
    let (_root, row) = notification_row();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);
    let mut notification = sample_notification();
    notification.actions = vec![Action {
        key: "default".to_string(),
        label: "Open conversation".to_string(),
    }];

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

    assert_eq!(child_count(&row.actions_box), 1);
    let button = row
        .actions_box
        .first_child()
        .and_downcast::<gtk::Button>()
        .expect("compact default action button");
    assert!(button.has_css_class("unixnotis-panel-default-action"));
    assert_eq!(button.tooltip_text().as_deref(), Some("Open notification"));
}

#[gtk::test]
fn confirmable_panel_action_requires_two_clicks_before_dispatch() {
    let (_root, row) = notification_row();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(2);
    let mut notification = sample_notification();
    notification.attribution = unixnotis_core::NotificationAttribution::associated(
        "Example Chat",
        "Example Chat",
        "org.example.Chat",
        "org.example.Chat",
        unixnotis_core::IdentityAssurance::SystemAssociated,
        unixnotis_core::InteractionPolicies::NATIVE_COMPATIBILITY,
        unixnotis_core::AttributionReason::ExactSystemExecutable,
        "protected executable association",
        "associated:system-app:org.example.Chat".to_string(),
    );
    notification.actions = vec![Action {
        key: "archive".to_string(),
        label: "Archive".to_string(),
    }];

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
        .and_downcast::<gtk::Button>()
        .expect("confirmable action button");
    button.emit_clicked();
    assert_eq!(button.label().as_deref(), Some("Confirm Archive"));
    assert!(
        command_rx.try_recv().is_err(),
        "first click must not invoke a confirmable action"
    );

    std::thread::sleep(std::time::Duration::from_millis(400));
    let context = gtk::glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }
    button.emit_clicked();
    assert!(matches!(
        command_rx.try_recv(),
        Ok(UiCommand::InvokeAction {
            notification,
            action_key,
            confirmed: true,
        }) if notification.id == 1
            && notification.generation == 1
            && action_key == "archive"
    ));

    std::thread::sleep(std::time::Duration::from_millis(400));
    let context = gtk::glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }
    button.emit_clicked();
    assert_eq!(button.label().as_deref(), Some("Confirm Archive"));
    assert!(
        command_rx.try_recv().is_err(),
        "third click must re-arm rather than dispatching"
    );
}

#[gtk::test]
fn historical_blank_default_action_has_no_control_or_activation() {
    let (_root, row) = notification_row();
    let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(2);
    let mut notification = sample_notification();
    notification.actions = vec![Action {
        key: "default".to_string(),
        label: String::new(),
    }];

    update_notification_row(
        &row,
        &row_data(Rc::new(notification), RowFlags::default()),
        &IconResolver::new(),
        &command_tx,
    );

    assert_eq!(child_count(&row.actions_box), 0);
    assert!(command_rx.try_recv().is_err());
}

#[gtk::test]
fn panel_keeps_two_primary_actions_and_moves_the_rest_into_more_menu() {
    let (_root, row) = notification_row();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(4);
    let mut notification = sample_notification();
    notification.actions = ["Open", "Archive", "Mute"]
        .into_iter()
        .map(|label| Action {
            key: label.to_ascii_lowercase(),
            label: label.to_string(),
        })
        .collect();

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

    assert_eq!(child_count(&row.actions_box), 3);
    assert!(row
        .actions_box
        .first_child()
        .is_some_and(|child| child.is::<gtk::Button>()));
    assert!(row
        .actions_box
        .last_child()
        .is_some_and(|child| child.is::<gtk::MenuButton>()));
    let menu = row
        .actions_box
        .last_child()
        .and_downcast::<gtk::MenuButton>()
        .expect("overflow menu");
    assert_eq!(menu.icon_name().as_deref(), Some("view-more-symbolic"));
    assert_eq!(menu.tooltip_text().as_deref(), Some("More actions"));
    assert!(menu.has_css_class("unixnotis-panel-action-overflow"));
    let popover = menu.popover().expect("overflow popover");
    let list = popover
        .child()
        .and_downcast::<gtk::Box>()
        .expect("overflow action list");
    assert!(list.has_css_class("unixnotis-panel-action-overflow-list"));
    let overflow = list
        .first_child()
        .and_downcast::<gtk::Button>()
        .expect("overflow action button");
    assert_eq!(overflow.label().as_deref(), Some("Mute"));
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
    assert_eq!(visible_action_count(&notification, false), 0);

    notification.actions.push(Action {
        key: "inline-reply".to_string(),
        label: "Reply".to_string(),
    });
    assert_eq!(visible_action_count(&notification, true), 2);
    notification.inline_reply.available = true;
    assert_eq!(visible_action_count(&notification, false), 0);
    assert_eq!(visible_action_count(&notification, true), 3);
    notification.inline_reply_policy = unixnotis_core::InlineReplyPolicy::Deny;
    assert_eq!(visible_action_count(&notification, true), 2);
}

#[test]
fn visible_action_count_includes_primary_and_overflow_actions() {
    let mut notification = sample_notification();
    notification.actions = ["Open", "Archive", "Mute"]
        .into_iter()
        .map(|label| Action {
            key: label.to_ascii_lowercase(),
            label: label.to_string(),
        })
        .collect();

    assert_eq!(visible_action_count(&notification, true), 3);
    assert_eq!(visible_action_count(&notification, false), 0);
}
