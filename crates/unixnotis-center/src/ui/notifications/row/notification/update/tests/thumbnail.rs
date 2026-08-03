//! Thumbnail visibility rules for notification rows

use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::{hooks, ImageData};

use crate::ui::icons::IconResolver;

use super::super::super::test_support::{
    notification_row, row_data, sample_notification, RowFlags,
};
use super::super::thumbnail::{
    notification_has_conversation_avatar, notification_has_sender_visual,
};
use super::{notification_has_thumbnail, update_notification_row};

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
    assert!(row.card.has_css_class(hooks::panel_card::HAS_THUMBNAIL));
    assert!(!row.card.has_css_class(hooks::panel_card::NO_THUMBNAIL));
}
