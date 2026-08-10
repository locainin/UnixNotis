//! Generic popup/panel visual-role matrix for reusable notification rows

use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::{hooks, ImageData};

use crate::ui::icons::IconResolver;

use super::super::super::test_support::{
    notification_row, row_data, sample_notification, RowFlags,
};
use super::super::thumbnail::{panel_lead_visual, PanelLeadVisual};
use super::super::update_notification_row;
use unixnotis_ui::presentation::NotificationPresentation;

#[test]
fn unresolved_conversation_avatar_follows_avatar_setting() {
    let mut notification = sample_notification();
    notification.attribution = unixnotis_core::NotificationAttribution::unresolved(
        "Example Chat",
        unixnotis_core::AttributionReason::MissingSenderEvidence,
        "no sender evidence",
        "unknown:example-chat".to_string(),
    );
    notification.image.sender_visual_role =
        unixnotis_core::NotificationVisualRole::ConversationAvatar;
    notification.image.sender_visual = avatar_pixel([255, 0, 0, 255]);
    let presentation = NotificationPresentation::from_view(&notification);

    assert_eq!(
        panel_lead_visual(&presentation, true, false),
        PanelLeadVisual::ConversationAvatar
    );
    assert_eq!(
        panel_lead_visual(&presentation, false, true),
        PanelLeadVisual::None
    );
}

#[gtk::test]
fn panel_conversation_avatar_obeys_avatar_setting_without_thumbnail_fallback() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.attribution = unixnotis_core::NotificationAttribution::unresolved(
        "Example Chat",
        unixnotis_core::AttributionReason::MissingSenderEvidence,
        "no sender evidence",
        "unknown:example-chat".to_string(),
    );
    notification.image.sender_visual_role =
        unixnotis_core::NotificationVisualRole::ConversationAvatar;
    notification.image.sender_visual = avatar_pixel([255, 0, 0, 255]);
    notification.image.content_image = ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![8, 9, 10, 255],
    };
    let notification = Rc::new(notification);
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    let enabled = row_data(
        Rc::clone(&notification),
        RowFlags {
            show_avatar: true,
            show_thumbnail: false,
            ..Default::default()
        },
    );
    update_notification_row(&row, &enabled, &IconResolver::new(), &command_tx);
    assert!(row.thumbnail.get_visible());
    assert!(row.thumbnail.paintable().is_some());

    let disabled = row_data(
        notification,
        RowFlags {
            show_avatar: false,
            show_thumbnail: true,
            ..Default::default()
        },
    );
    update_notification_row(&row, &disabled, &IconResolver::new(), &command_tx);
    assert!(row.thumbnail.get_visible());
    assert!(row.thumbnail.paintable().is_some());
    assert!(row
        .thumbnail
        .has_css_class(hooks::panel_card::CONTENT_IMAGE));
    assert!(!row
        .thumbnail
        .has_css_class(hooks::panel_card::SENDER_VISUAL));
}

#[gtk::test]
fn grouped_rows_keep_trust_chip_in_the_shared_application_header() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.attribution = unixnotis_core::NotificationAttribution::unresolved(
        "Example Chat",
        unixnotis_core::AttributionReason::MissingSenderEvidence,
        "no sender evidence",
        "unknown:example-chat".to_string(),
    );
    let data = row_data(Rc::new(notification), RowFlags::default());
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(!row.trust_chip.get_visible());
}

#[gtk::test]
fn identity_signature_is_set_for_owned_headers_and_cleared_for_grouped_rows() {
    let (_root, row) = notification_row();
    let notification = Rc::new(sample_notification());
    let mut standalone = row_data(notification.clone(), RowFlags::default());
    standalone.app_header_present = false;
    let grouped = row_data(notification, RowFlags::default());
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    update_notification_row(&row, &standalone, &IconResolver::new(), &command_tx);
    assert!(row.icon_sig.borrow().is_some());

    update_notification_row(&row, &grouped, &IconResolver::new(), &command_tx);
    assert!(row.icon_sig.borrow().is_none());
}

#[gtk::test]
fn grouped_rebind_invalidates_previous_identity_icon_request() {
    let (_root, row) = notification_row();
    let resolver = IconResolver::new();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    let mut notification = sample_notification();
    notification.attribution = unixnotis_core::NotificationAttribution::unresolved(
        "Example Application",
        unixnotis_core::AttributionReason::MissingSenderEvidence,
        "generic fixture",
        "unknown:example".to_string(),
    );
    notification.image.claimed_desktop_id = "org.example.Async.desktop".to_string();
    let notification = Rc::new(notification);

    let mut standalone = row_data(Rc::clone(&notification), RowFlags::default());
    standalone.app_header_present = false;
    let mut grouped = row_data(notification, RowFlags::default());
    grouped.app_header_present = true;

    update_notification_row(&row, &standalone, &resolver, &command_tx);
    update_notification_row(&row, &grouped, &resolver, &command_tx);

    assert!(row.icon_sig.borrow().is_none());
    assert!(row.icon.paintable().is_none());
    assert!(!row.icon.get_visible());
}

#[gtk::test]
fn malformed_conversation_avatar_does_not_reserve_a_panel_lead_slot() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.image.sender_visual_role =
        unixnotis_core::NotificationVisualRole::ConversationAvatar;
    // The role is present, but the raster has no valid dimensions or pixels
    notification.image.sender_visual = ImageData {
        width: 0,
        height: 0,
        data: vec![1, 2, 3, 255],
        ..ImageData::default()
    };
    let data = row_data(Rc::new(notification), RowFlags::default());
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(!row.thumbnail.get_visible());
    assert!(row.thumbnail.paintable().is_none());
    assert!(!row.card.has_css_class(hooks::panel_card::HAS_THUMBNAIL));
    assert!(row.card.has_css_class(hooks::panel_card::NO_THUMBNAIL));
}

#[gtk::test]
fn conversation_avatar_wins_panel_lead_slot_when_content_is_also_present() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.image.sender_visual_role =
        unixnotis_core::NotificationVisualRole::ConversationAvatar;
    notification.image.sender_visual = avatar_pixel([255, 0, 0, 255]);
    notification.image.content_image = ImageData {
        width: 2,
        height: 2,
        rowstride: 8,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: [9, 8, 7, 255].repeat(4),
    };
    let data = row_data(Rc::new(notification), RowFlags::default());
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(row.thumbnail.get_visible());
    assert!(row.thumbnail.paintable().is_some());
    assert!(!row
        .thumbnail
        .has_css_class(hooks::panel_card::CONTENT_IMAGE));
    assert!(!row
        .thumbnail
        .has_css_class(hooks::panel_card::SENDER_VISUAL));
}

#[gtk::test]
fn content_only_notification_stays_in_the_content_lead_lane() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.image.content_image = ImageData {
        width: 2,
        height: 2,
        rowstride: 8,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: [9, 8, 7, 255].repeat(4),
    };
    let data = row_data(
        Rc::new(notification),
        RowFlags {
            show_thumbnail: true,
            ..Default::default()
        },
    );
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(row.thumbnail.get_visible());
    assert!(row.thumbnail.paintable().is_some());
    assert!(row
        .thumbnail
        .has_css_class(hooks::panel_card::CONTENT_IMAGE));
    assert!(!row
        .thumbnail
        .has_css_class(hooks::panel_card::SENDER_VISUAL));
}

#[gtk::test]
fn malformed_conversation_avatar_falls_back_to_valid_content_media() {
    let (_root, row) = notification_row();
    let mut notification = sample_notification();
    notification.image.sender_visual_role =
        unixnotis_core::NotificationVisualRole::ConversationAvatar;
    notification.image.sender_visual = ImageData {
        width: 0,
        height: 0,
        data: vec![1, 2, 3, 255],
        ..ImageData::default()
    };
    notification.image.content_image = ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![9, 8, 7, 255],
    };
    let data = row_data(
        Rc::new(notification),
        RowFlags {
            show_avatar: true,
            show_thumbnail: true,
            ..Default::default()
        },
    );
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);

    update_notification_row(&row, &data, &IconResolver::new(), &command_tx);

    assert!(row.thumbnail.get_visible());
    assert!(row.thumbnail.paintable().is_some());
    assert!(row
        .thumbnail
        .has_css_class(hooks::panel_card::CONTENT_IMAGE));
    assert!(!row
        .thumbnail
        .has_css_class(hooks::panel_card::SENDER_VISUAL));
    assert!(row.card.has_css_class(hooks::panel_card::HAS_THUMBNAIL));

    // Disabling content thumbnails must not turn the malformed avatar into a fallback lane
    let mut hidden_content = sample_notification();
    hidden_content.image.sender_visual_role =
        unixnotis_core::NotificationVisualRole::ConversationAvatar;
    hidden_content.image.sender_visual = ImageData {
        width: 0,
        height: 0,
        data: vec![1, 2, 3, 255],
        ..ImageData::default()
    };
    hidden_content.image.content_image = ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![9, 8, 7, 255],
    };
    let hidden_data = row_data(
        Rc::new(hidden_content),
        RowFlags {
            show_avatar: true,
            show_thumbnail: false,
            ..Default::default()
        },
    );
    update_notification_row(&row, &hidden_data, &IconResolver::new(), &command_tx);
    assert!(row.thumbnail.paintable().is_none());
    assert!(!row.thumbnail.get_visible());
    assert!(!row
        .thumbnail
        .has_css_class(hooks::panel_card::CONTENT_IMAGE));
}

#[gtk::test]
fn rapid_avatar_replacement_clears_previous_paintable() {
    let (_root, row) = notification_row();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);
    let resolver = IconResolver::new();

    let mut first = sample_notification();
    first.image.sender_visual_role = unixnotis_core::NotificationVisualRole::ConversationAvatar;
    first.image.sender_visual = avatar_pixel([255, 0, 0, 255]);
    update_notification_row(
        &row,
        &row_data(Rc::new(first), RowFlags::default()),
        &resolver,
        &command_tx,
    );
    let first_pixels = paintable_rgba(&row.thumbnail).expect("first avatar pixels");

    let mut second = sample_notification();
    second.image.sender_visual_role = unixnotis_core::NotificationVisualRole::ConversationAvatar;
    second.image.sender_visual = avatar_pixel([0, 0, 255, 255]);
    update_notification_row(
        &row,
        &row_data(Rc::new(second), RowFlags::default()),
        &resolver,
        &command_tx,
    );
    let second_pixels = paintable_rgba(&row.thumbnail).expect("replacement avatar pixels");
    assert_ne!(first_pixels, second_pixels);

    let mut empty = sample_notification();
    empty.image.sender_visual_role = unixnotis_core::NotificationVisualRole::None;
    empty.image.sender_visual = ImageData::default();
    update_notification_row(
        &row,
        &row_data(Rc::new(empty), RowFlags::default()),
        &resolver,
        &command_tx,
    );
    assert!(row.thumbnail.paintable().is_none());
    assert!(!row.thumbnail.get_visible());
}

#[gtk::test]
fn burst_rebinding_does_not_retain_another_notification_visual() {
    let (_root, row) = notification_row();
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(2);
    let resolver = IconResolver::new();

    for index in 0..40 {
        let mut notification = sample_notification();
        let use_avatar = index % 2 == 0;
        let use_content = index % 3 == 0;
        if use_avatar {
            notification.image.sender_visual_role =
                unixnotis_core::NotificationVisualRole::ConversationAvatar;
            notification.image.sender_visual = avatar_pixel([index as u8, 2, 3, 255]);
        }
        if use_content {
            notification.image.content_image = ImageData {
                width: 2,
                height: 2,
                rowstride: 8,
                has_alpha: true,
                bits_per_sample: 8,
                channels: 4,
                data: [4, index as u8, 6, 255].repeat(4),
            };
        }
        let expected_visual = use_avatar || use_content;
        let show_thumbnail = use_content;
        let data = row_data(
            Rc::new(notification),
            RowFlags {
                show_thumbnail,
                ..Default::default()
            },
        );

        update_notification_row(&row, &data, &resolver, &command_tx);

        assert_eq!(row.thumbnail.get_visible(), expected_visual);
        assert_eq!(row.thumbnail.paintable().is_some(), expected_visual);
    }
}

fn avatar_pixel(pixel: [u8; 4]) -> ImageData {
    ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: pixel.to_vec(),
    }
}

fn paintable_rgba(image: &gtk::Image) -> Option<Vec<u8>> {
    let texture = image.paintable()?.downcast::<gtk::gdk::Texture>().ok()?;
    let width = usize::try_from(texture.width()).ok()?;
    let height = usize::try_from(texture.height()).ok()?;
    let stride = width.checked_mul(4)?;
    let mut pixels = vec![0; stride.checked_mul(height)?];
    gtk::gdk::prelude::TextureExtManual::download(&texture, &mut pixels, stride);
    Some(pixels)
}
