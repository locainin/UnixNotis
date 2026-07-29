//! Visual state updates for notification rows

use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::{hooks, Action, CutCorners, NotificationMetadataConfig, Urgency};

use crate::ui::icons::IconResolver;

use super::super::super::state::IconSignature;
use super::super::super::test_support::{
    child_count, notification_row, notification_row_with_receiver, row_data, sample_notification,
    RowFlags,
};
use super::update_notification_row;

#[test]
fn icon_signature_changes_when_trust_presentation_changes() {
    let verified = sample_notification();
    let mut suspicious = verified.clone();
    // Keep resolver inputs unchanged to isolate the trust-state regression
    suspicious.attribution.status = unixnotis_core::AttributionStatus::Conflict;

    assert_ne!(
        IconSignature::from(&verified),
        IconSignature::from(&suspicious),
        "trust changes must refresh a recycled row badge"
    );
}

#[gtk::test]
fn close_control_ignores_unbound_rows_and_keeps_the_bound_generation() {
    let (_root, row, mut command_rx) = notification_row_with_receiver();

    row.close_button.emit_clicked();
    assert!(
        command_rx.try_recv().is_err(),
        "an unbound recycled control must not dismiss notification zero"
    );

    row.notify_key.set(unixnotis_core::NotificationKey {
        id: 7,
        generation: 11,
    });
    row.close_button.emit_clicked();
    assert!(matches!(
        command_rx.try_recv(),
        Ok(crate::control::UiCommand::Dismiss(notification))
            if notification.id == 7 && notification.generation == 11
    ));
}

#[gtk::test]
fn update_notification_row_applies_state_classes_and_text() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.urgency = Urgency::Critical as u8;
    let notification = Rc::new(notification);
    let expected_key = notification.key();
    let data = row_data(
        notification,
        RowFlags {
            is_active: true,
            collapsed_group_preview: true,
            ..Default::default()
        },
    );
    let (command_tx, _rx) = tokio::sync::mpsc::channel(4);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(row.card.has_css_class(hooks::shared_state::CRITICAL));
    assert!(row.card.has_css_class(hooks::shared_state::ACTIVE));
    assert!(row
        .card
        .has_css_class(hooks::shared_state::COLLAPSED_GROUP_PREVIEW));
    assert!(row.card.has_css_class(hooks::panel_card::GROUPED));
    assert!(!row.app_label.get_visible());
    assert!(!row.icon.get_visible());
    assert!(row.header.get_visible());
    assert!(row.urgency_badge.get_visible());
    assert_eq!(row.urgency_badge.text().as_str(), "Critical");
    assert_eq!(row.app_label.text().as_str(), "demo");
    assert_eq!(row.summary_label.text().as_str(), "summary");
    assert_eq!(row.body_label.text().as_str(), "body");
    assert_eq!(row.notify_key.get(), expected_key);
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
    assert!(row.header.get_visible());
    assert!(row.close_button.get_visible());
    assert_eq!(row.app_label.text().as_str(), "demo");
    assert!(row.icon_sig.borrow().is_some());
}

#[gtk::test]
fn relay_singleton_shows_authenticated_source_and_secondary_app_label() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.attribution = unixnotis_core::NotificationAttribution::relay(
        "Signal",
        "Sent via /usr/bin/notify-send",
        "relay:notify-send:signal".to_string(),
    );
    let data = row_data(Rc::new(notification), RowFlags::default());
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert_eq!(row.app_label.text().as_str(), "Command-line notification");
    assert_eq!(row.secondary_claim.text().as_str(), "App label: Signal");
    assert!(row.secondary_claim.get_visible());
    assert!(!row.trust_chip.get_visible());
    assert!(row.card.has_css_class("relay"));
    assert!(!row.card.has_css_class("conflict"));
}

#[gtk::test]
fn panel_text_limits_keep_compact_rows_content_driven() {
    let (root, row) = notification_row();
    let close = descendant_with_class(root.upcast_ref(), "unixnotis-panel-close")
        .expect("panel close button");

    assert_eq!(row.summary_label.lines(), 1);
    assert_eq!(row.body_label.lines(), 3);
    assert_eq!(close.parent().as_ref(), Some(row.header.upcast_ref()));
}

#[gtk::test]
fn grouped_relay_row_hides_identity_details_owned_by_the_group_header() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.attribution = unixnotis_core::NotificationAttribution::relay(
        "Signal",
        "Sent via /usr/bin/notify-send",
        "relay:notify-send:signal".to_string(),
    );
    let data = row_data(
        Rc::new(notification),
        RowFlags {
            collapsed_group_preview: true,
            ..Default::default()
        },
    );
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(!row.app_label.get_visible());
    assert!(!row.secondary_claim.get_visible());
    assert!(!row.trust_chip.get_visible());
    assert!(!row.icon.get_visible());
    assert!(row.header.get_visible());
    assert!(row.close_button.get_visible());
}

#[gtk::test]
fn collapsed_group_preview_uses_one_content_surface() {
    let (root, row) = notification_row();
    let data = row_data(
        Rc::new(sample_notification()),
        RowFlags {
            collapsed_group_preview: true,
            ..Default::default()
        },
    );
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert_eq!(child_count(&root), 1);
    assert!(
        row.card
            .has_css_class(hooks::shared_state::COLLAPSED_GROUP_PREVIEW),
        "the single readable surface should retain collapsed preview state"
    );
}

#[gtk::test]
fn recycled_standalone_row_clears_identity_cache_when_it_becomes_grouped() {
    let (_root, row) = notification_row();
    let notification = Rc::new(sample_notification());
    let standalone = row_data(notification.clone(), RowFlags::default());
    let grouped = row_data(
        notification,
        RowFlags {
            collapsed_group_preview: true,
            ..Default::default()
        },
    );
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    update_notification_row(&row, &standalone, &IconResolver::new(), &command_tx);
    assert!(
        row.icon_sig.borrow().is_some(),
        "standalone rows should cache their resolved identity icon"
    );

    update_notification_row(&row, &grouped, &IconResolver::new(), &command_tx);
    assert!(
        row.icon_sig.borrow().is_none(),
        "grouped rows must release identity state owned by their group header"
    );
}

#[gtk::test]
fn compact_rows_place_relative_time_in_the_non_overlapping_header_lane() {
    let (_root, row) = notification_row();
    let notification = Rc::new(sample_notification());
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);
    let current = row_data(notification.clone(), RowFlags::default());

    update_notification_row(&row, &current, &IconResolver::new(), &command_tx);

    assert!(!row.meta_top.get_visible());
    assert!(row.time_badge.get_visible());
    assert!(!row.meta_label.get_visible());
    assert!(!row.footer.get_visible());
    assert!(!row.footer_left.get_visible());
    assert!(!row.footer_right.get_visible());

    let mut missing_time = row_data(notification, RowFlags::default());
    missing_time.presentation.received_at_ms = 0;
    update_notification_row(&row, &missing_time, &IconResolver::new(), &command_tx);

    assert!(!row.meta_top.get_visible());
    assert!(!row.time_badge.get_visible());
}

fn descendant_with_class(widget: &gtk::Widget, class_name: &str) -> Option<gtk::Widget> {
    if widget.has_css_class(class_name) {
        return Some(widget.clone());
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(found) = descendant_with_class(&current, class_name) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}

#[gtk::test]
fn popup_suppression_reason_is_rendered_from_the_committed_decision() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.popup_decision = unixnotis_core::PopupDecisionRecord {
        admission_at_commit: unixnotis_core::PopupAdmissionView::Dnd,
        decided_at_unix_ms: 1_000,
        delivery_stage: unixnotis_core::PopupDeliveryStage::Suppressed,
        ..unixnotis_core::PopupDecisionRecord::default()
    };
    let data = row_data(Rc::new(notification), RowFlags::default());
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert_eq!(
        row.popup_status.text().as_str(),
        "Not shown — Do Not Disturb was enabled"
    );
    assert!(row.popup_status.get_visible());
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
fn grouped_rows_keep_cut_corners_only_on_the_outer_bottom_edge() {
    let (_root, row) = notification_row();
    let corners = CutCorners {
        top_left: 8,
        top_right: 9,
        bottom_right: 10,
        bottom_left: 11,
    };
    let mut middle = row_data(
        Rc::new(sample_notification()),
        RowFlags {
            card_corners: corners,
            ..Default::default()
        },
    );
    middle.expanded = true;
    let (command_tx, _rx) = tokio::sync::mpsc::channel(1);

    update_notification_row(&row, &middle, &IconResolver::new(), &command_tx);
    assert_eq!(row.card_plate.corners(), CutCorners::default());

    let mut last = middle;
    last.group_last = true;
    update_notification_row(&row, &last, &IconResolver::new(), &command_tx);
    assert_eq!(
        row.card_plate.corners(),
        CutCorners {
            bottom_right: 10,
            bottom_left: 11,
            ..CutCorners::default()
        }
    );
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
