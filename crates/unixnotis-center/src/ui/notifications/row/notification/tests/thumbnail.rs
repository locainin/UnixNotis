//! Thumbnail visibility rules for notification rows

use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::hooks;

use crate::ui::icons::IconResolver;

use super::test_support::{notification_row, row_data, sample_notification, RowFlags};
use super::update::{notification_has_thumbnail, update_notification_row};

#[test]
fn notification_thumbnail_only_uses_real_image_sources() {
    let mut notification = sample_notification();
    assert!(!notification_has_thumbnail(&notification));

    notification.image.image_path = "/tmp/demo.png".to_string();
    assert!(notification_has_thumbnail(&notification));
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
    notification.image.image_path = "/tmp/demo.png".to_string();
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
