use unixnotis_core::{Action, AttributionClass, ImageData, NotificationAttribution};

use super::super::{PopupEntryViewModel, PopupKind, ThumbnailKind};
use super::support::notification;

#[test]
fn view_model_formats_relative_time_without_losing_original_age() {
    let mut view = notification();
    view.received_at_unix_seconds = 1_000;

    assert_eq!(
        PopupEntryViewModel::for_notification_at(&view, 1_030).timestamp_label,
        "now"
    );
    assert_eq!(
        PopupEntryViewModel::for_notification_at(&view, 1_120).timestamp_label,
        "2m"
    );
    assert_eq!(
        PopupEntryViewModel::for_notification_at(&view, 8_200).timestamp_label,
        "2h"
    );
    assert_eq!(
        PopupEntryViewModel::for_notification_at(&view, 173_800).timestamp_label,
        "2d"
    );

    view.received_at_unix_seconds = 0;
    assert_eq!(
        PopupEntryViewModel::for_notification_at(&view, 173_800).timestamp_label,
        "now"
    );
    view.received_at_unix_seconds = 200_000;
    assert_eq!(
        PopupEntryViewModel::for_notification_at(&view, 173_800).timestamp_label,
        "now"
    );
}

#[test]
fn utility_layout_keeps_only_one_safe_action() {
    let mut view = notification();
    view.actions = vec![
        Action {
            key: "default".to_string(),
            label: "Open".to_string(),
        },
        Action {
            key: "folder".to_string(),
            label: "Open folder".to_string(),
        },
    ];

    let model = PopupEntryViewModel::for_notification_at(&view, 1_000);

    assert_eq!(model.kind, PopupKind::Utility);
    assert_eq!(model.actions.len(), 1);
    assert_eq!(model.actions[0].key, "default");
}

#[test]
fn weak_attribution_hides_every_application_directed_action() {
    let mut view = notification();
    view.attribution = NotificationAttribution::unknown(
        "Signal",
        "source /tmp/fake",
        "unknown:signal".to_string(),
    );
    view.actions.push(Action {
        key: "default".to_string(),
        label: "Open".to_string(),
    });

    let model = PopupEntryViewModel::for_notification_at(&view, 1_000);

    assert!(model.actions.is_empty());
}

#[test]
fn square_icon_data_is_hidden_unless_the_category_is_media() {
    let mut view = notification();
    view.image.has_image_data = true;
    view.image.image_data = ImageData {
        width: 64,
        height: 64,
        ..ImageData::default()
    };

    let utility = PopupEntryViewModel::for_notification_at(&view, 1_000);
    assert_eq!(utility.thumbnail, ThumbnailKind::None);

    view.category = "image.photo".to_string();
    let media = PopupEntryViewModel::for_notification_at(&view, 1_000);
    assert_eq!(media.thumbnail, ThumbnailKind::Content);
}

#[test]
fn thumbnail_requires_real_image_data_or_a_nonempty_path() {
    let mut view = notification();
    assert_eq!(
        PopupEntryViewModel::for_notification_at(&view, 1_000).thumbnail,
        ThumbnailKind::None
    );

    view.image.image_path = "/tmp/content.png".to_string();
    assert_eq!(
        PopupEntryViewModel::for_notification_at(&view, 1_000).thumbnail,
        ThumbnailKind::Content
    );
}

#[test]
fn either_badge_source_match_suppresses_duplicate_decoration() {
    let mut icon_match = notification();
    icon_match.attribution.badge_icon = "example".to_string();
    icon_match.image.has_image_data = true;
    icon_match.image.icon_name = "example".to_string();
    icon_match.image.image_data = ImageData {
        width: 160,
        height: 90,
        ..ImageData::default()
    };
    assert_eq!(
        PopupEntryViewModel::for_notification_at(&icon_match, 1_000).thumbnail,
        ThumbnailKind::None
    );

    let mut path_match = icon_match;
    path_match.image.icon_name = "different".to_string();
    path_match.image.image_path = "example".to_string();
    assert_eq!(
        PopupEntryViewModel::for_notification_at(&path_match, 1_000).thumbnail,
        ThumbnailKind::None
    );

    let mut no_match = path_match;
    no_match.image.image_path = "different".to_string();
    assert_eq!(
        PopupEntryViewModel::for_notification_at(&no_match, 1_000).thumbnail,
        ThumbnailKind::Content
    );
}

#[test]
fn decorative_square_detection_uses_every_dimension_guard_and_exact_boundary() {
    let mut view = notification();
    view.image.has_image_data = true;

    for (width, height, expected) in [
        (0, 0, ThumbnailKind::Content),
        (96, 72, ThumbnailKind::Content),
        (128, 128, ThumbnailKind::None),
        (129, 129, ThumbnailKind::Content),
    ] {
        view.image.image_data = ImageData {
            width,
            height,
            ..ImageData::default()
        };
        assert_eq!(
            PopupEntryViewModel::for_notification_at(&view, 1_000).thumbnail,
            expected,
            "{width}x{height} should have the intended decoration classification"
        );
    }
}

#[test]
fn square_path_content_is_not_mistaken_for_embedded_icon_data() {
    let mut view = notification();
    view.image.image_path = "/tmp/content.png".to_string();
    view.image.image_data = ImageData {
        width: 64,
        height: 64,
        ..ImageData::default()
    };

    assert_eq!(
        PopupEntryViewModel::for_notification_at(&view, 1_000).thumbnail,
        ThumbnailKind::Content
    );
}

#[test]
fn conflicting_claim_uses_warning_layout_and_drops_actions() {
    let mut view = notification();
    view.category = "im.received".to_string();
    view.attribution = NotificationAttribution::associated(
        "Unknown application",
        "",
        "dialog-warning-symbolic",
        "Claims to be Signal",
        AttributionClass::Conflict,
        true,
        "conflict:signal".to_string(),
    );
    view.actions.push(Action {
        key: "default".to_string(),
        label: "Open".to_string(),
    });

    let model = PopupEntryViewModel::for_notification_at(&view, 1_000);

    assert_eq!(model.kind, PopupKind::Warning);
    assert!(model.actions.is_empty());
}
