//! Visual state updates for notification rows

use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::{hooks, Action, CutCorners, NotificationMetadataConfig, Urgency};

use crate::ui::icons::IconResolver;

use super::super::super::test_support::{
    notification_row, row_data, sample_notification, RowFlags,
};
use super::update_notification_row;

#[gtk::test]
fn update_notification_row_applies_state_classes_and_text() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.urgency = Urgency::Critical as u8;
    let notification = Rc::new(notification);
    let data = row_data(
        notification,
        RowFlags {
            is_active: true,
            stacked: true,
            stack_depth: 2,
            ..Default::default()
        },
    );
    let (command_tx, _rx) = tokio::sync::mpsc::channel(4);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(row.card.has_css_class(hooks::shared_state::CRITICAL));
    assert!(row.card.has_css_class(hooks::shared_state::ACTIVE));
    assert!(row.card.has_css_class(hooks::shared_state::STACKED));
    assert!(row.card.has_css_class(hooks::panel_card::GROUP_COLLAPSED));
    assert!(!row.card.has_css_class(hooks::panel_card::GROUP_EXPANDED));
    assert!(!row.app_label.get_visible());
    assert!(!row.icon.get_visible());
    assert!(row.urgency_badge.get_visible());
    assert_eq!(row.urgency_badge.text().as_str(), "Critical");
    assert_eq!(row.app_label.text().as_str(), "demo");
    assert_eq!(row.summary_label.text().as_str(), "summary");
    assert_eq!(row.body_label.text().as_str(), "body");
    assert_eq!(row.notify_id.get(), 1);
    assert!(row.icon_sig.borrow().is_none());
}

#[gtk::test]
fn recycled_panel_row_hides_critical_badge_after_urgency_returns_to_normal() {
    let (_root, row) = notification_row();
    let mut critical = sample_notification();
    critical.urgency = Urgency::Critical as u8;
    let critical = row_data(Rc::new(critical), RowFlags::default());
    let normal = row_data(Rc::new(sample_notification()), RowFlags::default());
    let (command_tx, _rx) = tokio::sync::mpsc::channel(4);

    update_notification_row(&row, &critical, &IconResolver::new(), &command_tx);
    assert!(row.urgency_badge.get_visible());

    update_notification_row(&row, &normal, &IconResolver::new(), &command_tx);
    assert!(!row.card.has_css_class(hooks::shared_state::CRITICAL));
    assert!(!row.urgency_badge.get_visible());
}

#[gtk::test]
fn single_notification_row_keeps_its_identity_visible_without_a_group_header() {
    let (_root, row) = notification_row();
    let data = row_data(Rc::new(sample_notification()), RowFlags::default());
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(row.app_label.get_visible());
    assert_eq!(row.app_label.text().as_str(), "demo");
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
    let data = row_data(
        Rc::new(notification),
        RowFlags {
            show_metadata: true,
            show_thumbnail: true,
            ..Default::default()
        },
    );
    let (command_tx, _rx) = tokio::sync::mpsc::channel(4);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(row.meta_top.get_visible());
    assert!(row.footer.get_visible());
    assert!(row.meta_label.get_visible());
    assert_eq!(row.meta_label.text().as_str(), "NOTICE");
    assert!(row.time_badge.get_visible());
    assert_eq!(row.footer_left.text().as_str(), "TRANSIENT");
    assert!(row.footer_right.get_visible());
    assert_eq!(row.footer_right.text().as_str(), "1 ACTION");
}

#[gtk::test]
fn update_notification_row_applies_custom_metadata_and_corner_geometry() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.actions = vec![
        Action {
            key: "open".to_string(),
            label: "Open".to_string(),
        },
        Action {
            key: "save".to_string(),
            label: "Save".to_string(),
        },
    ];
    let corners = CutCorners {
        top_left: 18,
        bottom_right: 12,
        ..CutCorners::default()
    };
    let metadata = NotificationMetadataConfig {
        normal_label: "INFO".to_string(),
        history_label: "ARCHIVE".to_string(),
        action_count_many: "{count} OPTIONS".to_string(),
        ..NotificationMetadataConfig::default()
    };
    let data = row_data(
        Rc::new(notification),
        RowFlags {
            show_metadata: true,
            metadata: Some(metadata),
            card_corners: corners,
            ..Default::default()
        },
    );
    let (command_tx, _rx) = tokio::sync::mpsc::channel(4);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert_eq!(row.meta_label.text().as_str(), "INFO");
    assert_eq!(row.footer_left.text().as_str(), "ARCHIVE");
    assert_eq!(row.footer_right.text().as_str(), "2 OPTIONS");
    assert_eq!(row.card_plate.corners(), corners);
}

#[gtk::test]
fn update_notification_row_marks_an_empty_action_set_as_unavailable() {
    let (_root, row) = notification_row();
    let data = row_data(Rc::new(sample_notification()), RowFlags::default());
    let (command_tx, _rx) = tokio::sync::mpsc::channel(1);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(!row.card.has_css_class(hooks::panel_card::HAS_ACTIONS));
    assert!(row.card.has_css_class(hooks::panel_card::NO_ACTIONS));
}

#[gtk::test]
fn update_notification_row_hides_metadata_labels_with_empty_custom_copy() {
    let (_root, row) = notification_row();
    let metadata = NotificationMetadataConfig {
        critical_label: String::new(),
        low_label: String::new(),
        normal_label: String::new(),
        relative_now: String::new(),
        relative_minutes: String::new(),
        relative_hours: String::new(),
        relative_days: String::new(),
        transient_label: String::new(),
        live_label: String::new(),
        history_label: String::new(),
        action_count_one: String::new(),
        action_count_many: String::new(),
    };
    let data = row_data(
        Rc::new(sample_notification()),
        RowFlags {
            show_metadata: true,
            metadata: Some(metadata),
            ..Default::default()
        },
    );
    let (command_tx, _rx) = tokio::sync::mpsc::channel(1);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(!row.meta_label.get_visible());
    assert!(!row.time_badge.get_visible());
    assert!(!row.footer_left.get_visible());
    assert!(!row.footer_right.get_visible());
}
