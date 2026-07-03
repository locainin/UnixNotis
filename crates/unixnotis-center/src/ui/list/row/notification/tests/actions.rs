//! Action button update rules for notification rows

use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::{hooks, Action};

use crate::dbus::UiCommand;
use crate::ui::icons::IconResolver;

use super::test_support::{child_count, notification_row, row_data, sample_notification};
use super::update::update_notification_row;

#[gtk::test]
fn update_notification_row_rebuilds_actions_only_when_signature_changes() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.actions = vec![Action {
        key: "open".to_string(),
        label: "Open".to_string(),
    }];
    let data = row_data(Rc::new(notification.clone()), true, false, 0, false, true);
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

    notification.actions[0].label = "Open notification details now".to_string();
    let data = row_data(Rc::new(notification), true, false, 0, false, true);
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
    let data = row_data(Rc::new(notification), true, false, 0, false, true);
    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert_eq!(child_count(&row.actions_box), 1);
    assert_eq!(row.action_cache.borrow()[0].0, "reply");
}

#[gtk::test]
fn update_notification_row_action_button_sends_command_once_per_click_window() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.actions = vec![Action {
        key: "open".to_string(),
        label: "Open".to_string(),
    }];
    let data = row_data(Rc::new(notification), true, false, 0, false, false);
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
