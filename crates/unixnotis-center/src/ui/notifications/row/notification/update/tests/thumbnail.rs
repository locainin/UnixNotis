//! Thumbnail visibility rules for notification rows

use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::{hooks, ImageData};
use unixnotis_ui::presentation::NotificationPresentation;

use crate::ui::icons::IconResolver;

use super::super::super::test_support::{
    notification_row, row_data, sample_notification, RowFlags,
};
use super::super::thumbnail::{
    has_content_thumbnail, has_conversation_avatar, has_sender_visual, panel_lead_visual,
    PanelLeadVisual,
};
use super::update_notification_row;

fn notification_has_thumbnail(notification: &unixnotis_core::NotificationView) -> bool {
    // Keep presentation construction in the mirrored test helper, not production code
    has_content_thumbnail(&NotificationPresentation::from_view(notification))
}

fn notification_has_conversation_avatar(notification: &unixnotis_core::NotificationView) -> bool {
    has_conversation_avatar(&NotificationPresentation::from_view(notification))
}

fn notification_has_sender_visual(notification: &unixnotis_core::NotificationView) -> bool {
    has_sender_visual(&NotificationPresentation::from_view(notification))
}

#[test]
fn notification_thumbnail_only_uses_real_image_sources() {
    let mut notification = sample_notification();
    assert!(!notification_has_thumbnail(&notification));

    notification.image.content_image = ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![1, 2, 3, 4],
    };
    assert!(notification_has_thumbnail(&notification));
}

#[test]
fn conversation_avatar_is_a_separate_thumbnail_source() {
    let mut notification = sample_notification();
    notification.image.sender_visual_role =
        unixnotis_core::NotificationVisualRole::ConversationAvatar;
    notification.image.sender_visual = ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![255, 0, 0, 255],
    };

    assert!(notification_has_conversation_avatar(&notification));
    assert!(!notification_has_thumbnail(&notification));
}

#[test]
fn application_visual_is_a_decorative_thumbnail_source() {
    let mut notification = sample_notification();
    notification.image.sender_visual_role =
        unixnotis_core::NotificationVisualRole::ApplicationProvidedIcon;
    notification.image.sender_visual = ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![0, 255, 0, 255],
    };

    assert!(notification_has_sender_visual(&notification));
    assert!(!notification_has_conversation_avatar(&notification));
    assert!(!notification_has_thumbnail(&notification));
}

#[test]
fn conversation_avatar_has_priority_over_content_thumbnail() {
    let mut notification = sample_notification();
    notification.image.sender_visual_role =
        unixnotis_core::NotificationVisualRole::ConversationAvatar;
    notification.image.sender_visual = ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![255, 0, 0, 255],
    };
    notification.image.content_image = notification.image.sender_visual.clone();
    let presentation = NotificationPresentation::from_view(&notification);

    assert_eq!(
        panel_lead_visual(&presentation, true, true),
        PanelLeadVisual::ConversationAvatar
    );
    assert_eq!(
        panel_lead_visual(&presentation, false, true),
        PanelLeadVisual::ContentImage
    );
}

#[gtk::test]
fn update_notification_row_hides_optional_text_and_thumbnail_when_absent() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.summary = "   ".to_string();
    notification.body.clear();
    let data = row_data(
        Rc::new(notification),
        RowFlags {
            show_thumbnail: true,
            ..Default::default()
        },
    );
    let (command_tx, _rx) = tokio::sync::mpsc::channel(4);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(!row.summary_label.get_visible());
    assert!(!row.body_label.get_visible());
    assert!(!row.thumbnail.get_visible());
    assert!(!row.card.has_css_class(hooks::panel_card::HAS_THUMBNAIL));
    assert!(row.card.has_css_class(hooks::panel_card::NO_THUMBNAIL));
}

#[gtk::test]
fn update_notification_row_shows_thumbnail_when_config_and_image_allow_it() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.image.content_image = ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![1, 2, 3, 4],
    };
    let data = row_data(
        Rc::new(notification),
        RowFlags {
            show_thumbnail: true,
            ..Default::default()
        },
    );
    let (command_tx, _rx) = tokio::sync::mpsc::channel(4);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(row.thumbnail.get_visible());
    assert!(row
        .thumbnail
        .has_css_class(hooks::panel_card::CONTENT_IMAGE));
    assert!(row.card.has_css_class(hooks::panel_card::HAS_THUMBNAIL));
    assert!(!row.card.has_css_class(hooks::panel_card::NO_THUMBNAIL));
}

#[gtk::test]
fn conversation_avatar_uses_the_master_panel_lead_slot_by_default() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.image.sender_visual_role =
        unixnotis_core::NotificationVisualRole::ConversationAvatar;
    notification.image.sender_visual = ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![255, 0, 0, 255],
    };
    let data = row_data(Rc::new(notification), RowFlags::default());
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert_eq!(row.thumbnail.pixel_size(), 56);
    assert_eq!(row.thumbnail.width_request(), 56);
    assert_eq!(row.thumbnail.height_request(), 56);
    assert!(row.thumbnail.get_visible());
    assert!(row.thumbnail.paintable().is_some());
    assert!(!row.thumbnail.has_css_class("unixnotis-panel-sender-visual"));
}

#[gtk::test]
fn disabled_notification_avatars_suppress_conversation_lead_visual() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.image.sender_visual_role =
        unixnotis_core::NotificationVisualRole::ConversationAvatar;
    notification.image.sender_visual = ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![255, 0, 0, 255],
    };
    let data = row_data(
        Rc::new(notification),
        RowFlags {
            show_avatar: false,
            ..Default::default()
        },
    );
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(row.thumbnail.paintable().is_none());
    assert!(!row.thumbnail.get_visible());
    assert!(row.card.has_css_class(hooks::panel_card::NO_THUMBNAIL));
}

#[gtk::test]
fn collapsed_and_expanded_group_rows_keep_conversation_avatar() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.image.sender_visual_role =
        unixnotis_core::NotificationVisualRole::ConversationAvatar;
    notification.image.sender_visual = ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![255, 0, 0, 255],
    };
    let notification = Rc::new(notification);
    let collapsed = row_data(
        Rc::clone(&notification),
        RowFlags {
            collapsed_group_preview: true,
            ..Default::default()
        },
    );
    let expanded = row_data(notification, RowFlags::default());
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    update_notification_row(&row, &collapsed, &IconResolver::new(), &command_tx);
    assert!(row.thumbnail.get_visible());
    assert!(row.thumbnail.paintable().is_some());

    update_notification_row(&row, &expanded, &IconResolver::new(), &command_tx);
    assert!(row.thumbnail.get_visible());
    assert!(row.thumbnail.paintable().is_some());
}

#[gtk::test]
fn historical_empty_avatar_role_does_not_create_a_blank_lead_slot() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.image.sender_visual_role =
        unixnotis_core::NotificationVisualRole::ConversationAvatar;
    let data = row_data(Rc::new(notification), RowFlags::default());
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(!row.thumbnail.get_visible());
    assert!(row.thumbnail.paintable().is_none());
    assert!(row.card.has_css_class(hooks::panel_card::NO_THUMBNAIL));
}

#[gtk::test]
fn rebinding_avatar_row_to_history_clears_the_paintable_and_slot() {
    let (_root, row) = notification_row();
    let mut active = sample_notification();
    active.image.sender_visual_role = unixnotis_core::NotificationVisualRole::ConversationAvatar;
    active.image.sender_visual = ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![255, 0, 0, 255],
    };
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);
    let active_data = row_data(Rc::new(active), RowFlags::default());

    update_notification_row(&row, &active_data, &IconResolver::new(), &command_tx);
    assert!(row.thumbnail.paintable().is_some());
    assert!(row.thumbnail.get_visible());

    let mut history = sample_notification();
    history.image.sender_visual_role = unixnotis_core::NotificationVisualRole::None;
    let history_data = row_data(Rc::new(history), RowFlags::default());

    update_notification_row(&row, &history_data, &IconResolver::new(), &command_tx);

    assert!(row.thumbnail.paintable().is_none());
    assert!(!row.thumbnail.get_visible());
    assert!(row.card.has_css_class(hooks::panel_card::NO_THUMBNAIL));
}

#[gtk::test]
fn content_thumbnail_setting_does_not_hide_conversation_avatar() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.image.sender_visual_role =
        unixnotis_core::NotificationVisualRole::ConversationAvatar;
    notification.image.sender_visual = ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![255, 0, 0, 255],
    };
    let data = row_data(Rc::new(notification), RowFlags::default());
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(row.thumbnail.get_visible());
    assert!(row.card.has_css_class(hooks::panel_card::HAS_THUMBNAIL));
}
