//! Visual state updates for notification rows

use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::{hooks, Action, Urgency};

use crate::ui::icons::IconResolver;

use super::test_support::{notification_row, row_data, sample_notification};
use super::update::update_notification_row;

#[gtk::test]
fn update_notification_row_applies_state_classes_and_text() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.urgency = Urgency::Critical as u8;
    let notification = Rc::new(notification);
    let data = row_data(notification, true, true, 2, false, false);
    let (command_tx, _rx) = tokio::sync::mpsc::channel(4);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(row.card.has_css_class(hooks::shared_state::CRITICAL));
    assert!(row.card.has_css_class(hooks::shared_state::ACTIVE));
    assert!(row.card.has_css_class(hooks::shared_state::STACKED));
    assert!(row.card.has_css_class(hooks::panel_card::GROUP_COLLAPSED));
    assert!(!row.card.has_css_class(hooks::panel_card::GROUP_EXPANDED));
    assert!(row.stack_ghost_1.get_visible());
    assert!(row.stack_ghost_2.get_visible());
    assert_eq!(row.app_label.text().as_str(), "demo");
    assert_eq!(row.summary_label.text().as_str(), "summary");
    assert_eq!(row.body_label.text().as_str(), "body");
    assert_eq!(row.notify_id.get(), 1);
    assert!(row.icon_sig.borrow().is_some());
}

#[gtk::test]
fn update_notification_row_shows_metadata_lanes_and_footer_state() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.is_transient = true;
    notification.actions = vec![Action {
        key: "open".to_string(),
        label: "Open".to_string(),
    }];
    let data = row_data(Rc::new(notification), false, false, 0, true, true);
    let (command_tx, _rx) = tokio::sync::mpsc::channel(4);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(row.meta_top.get_visible());
    assert!(row.footer.get_visible());
    assert!(row.meta_label.get_visible());
    assert_eq!(row.meta_label.text().as_str(), "NOTICE");
    assert!(row.time_badge.get_visible());
    assert_eq!(row.footer_left.text().as_str(), "TRANSIENT");
    assert!(row.footer_right.get_visible());
    assert_eq!(row.footer_right.text().as_str(), "1 ACTIONS");
}
