use unixnotis_core::{Action, AttributionReason, ImageData, NotificationAttribution};

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
fn utility_layout_moves_extra_safe_actions_into_overflow() {
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
        Action {
            key: "archive".to_string(),
            label: "Archive".to_string(),
        },
        Action {
            key: "mute".to_string(),
            label: "Mute".to_string(),
        },
    ];

    let model = PopupEntryViewModel::for_notification_at(&view, 1_000);

    assert_eq!(model.kind, PopupKind::Utility);
    assert_eq!(model.default_action_key.as_deref(), Some("default"));
    assert_eq!(model.primary_actions.len(), 2);
    assert_eq!(model.primary_actions[0].key, "default");
    assert_eq!(model.primary_actions[1].key, "folder");
    assert_eq!(model.overflow_actions.len(), 2);
    assert_eq!(model.overflow_actions[0].key, "archive");
    assert_eq!(model.overflow_actions[1].key, "mute");
}

#[test]
fn blank_default_action_is_clickable_without_becoming_a_visible_control() {
    let mut view = notification();
    view.actions.push(Action {
        key: "default".to_string(),
        label: String::new(),
    });

    let model = PopupEntryViewModel::for_notification_at(&view, 1_000);

    assert_eq!(model.default_action_key.as_deref(), Some("default"));
    assert!(model.primary_actions.is_empty());
    assert!(model.overflow_actions.is_empty());
}

#[test]
fn weak_attribution_hides_every_application_directed_action() {
    let mut view = notification();
    view.attribution = NotificationAttribution::unresolved(
        "Example Chat",
        AttributionReason::NoDesktopCandidate,
        "source /tmp/fake",
        "unknown:example-chat".to_string(),
    );
    view.actions.push(Action {
        key: "default".to_string(),
        label: "Open".to_string(),
    });

    let model = PopupEntryViewModel::for_notification_at(&view, 1_000);

    assert!(model.primary_actions.is_empty());
    assert!(model.overflow_actions.is_empty());
}

#[test]
fn user_associated_attribution_hides_application_directed_actions() {
    let mut view = notification();
    view.attribution = NotificationAttribution::recognized(
        "User application",
        "User application",
        "org.example.UserApplication",
        "user-application",
        AttributionReason::ExactUserExecutable,
        "user desktop association",
        "user-desktop:org.example.UserApplication".to_string(),
    );
    view.actions.push(Action {
        key: "default".to_string(),
        label: "Open".to_string(),
    });

    let model = PopupEntryViewModel::for_notification_at(&view, 1_000);

    assert!(model.primary_actions.is_empty());
    assert!(model.overflow_actions.is_empty());
}

#[test]
fn communication_avatar_is_not_suppressed_as_decoration() {
    let mut view = notification();
    view.category = "im.received".to_string();
    view.image.content_image = ImageData {
        width: 64,
        height: 64,
        data: vec![0; 64 * 64 * 4],
        ..ImageData::default()
    };

    assert_eq!(
        PopupEntryViewModel::for_notification_at(&view, 1_000).thumbnail,
        ThumbnailKind::Content
    );
}

#[test]
fn thumbnail_requires_real_image_data() {
    let mut view = notification();
    assert_eq!(
        PopupEntryViewModel::for_notification_at(&view, 1_000).thumbnail,
        ThumbnailKind::None
    );

    view.image.content_image = ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        channels: 4,
        bits_per_sample: 8,
        data: vec![1, 2, 3, 4],
        ..ImageData::default()
    };
    assert_eq!(
        PopupEntryViewModel::for_notification_at(&view, 1_000).thumbnail,
        ThumbnailKind::Content
    );
}

#[test]
fn app_icon_name_never_suppresses_real_content_image_data() {
    let mut icon_match = notification();
    icon_match.attribution.badge_icon = "example".to_string();
    icon_match.image.badge_icon = "example".to_string();
    icon_match.image.content_image = ImageData {
        width: 160,
        height: 90,
        data: vec![0; 160 * 90 * 4],
        ..ImageData::default()
    };
    assert_eq!(
        PopupEntryViewModel::for_notification_at(&icon_match, 1_000).thumbnail,
        ThumbnailKind::Content
    );

    let mut path_match = notification();
    path_match.attribution.badge_icon = "example".to_string();
    path_match.image.content_image = ImageData::default();
    assert_eq!(
        PopupEntryViewModel::for_notification_at(&path_match, 1_000).thumbnail,
        ThumbnailKind::None
    );

    let mut no_match = path_match;
    no_match.image.content_image = ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        channels: 4,
        bits_per_sample: 8,
        data: vec![1, 2, 3, 4],
        ..ImageData::default()
    };
    assert_eq!(
        PopupEntryViewModel::for_notification_at(&no_match, 1_000).thumbnail,
        ThumbnailKind::Content
    );
}

#[test]
fn invalid_image_dimensions_do_not_create_thumbnail_content() {
    let mut view = notification();
    for (width, height) in [(0, 0), (64, 64), (96, 72), (128, 128), (129, 129)] {
        view.image.content_image = ImageData {
            width,
            height,
            data: if width > 0 && height > 0 {
                vec![0; 4]
            } else {
                Vec::new()
            },
            ..ImageData::default()
        };
        let expected = if width > 0 && height > 0 {
            ThumbnailKind::Content
        } else {
            ThumbnailKind::None
        };
        assert_eq!(
            PopupEntryViewModel::for_notification_at(&view, 1_000).thumbnail,
            expected
        );
    }
}

#[test]
fn square_content_is_rendered_as_notification_content() {
    let mut view = notification();
    view.image.content_image = ImageData {
        width: 64,
        height: 64,
        data: vec![0; 64 * 64 * 4],
        ..ImageData::default()
    };

    assert_eq!(
        PopupEntryViewModel::for_notification_at(&view, 1_000).thumbnail,
        ThumbnailKind::Content
    );
}

#[test]
fn conflicting_claim_keeps_communication_layout_and_drops_actions() {
    let mut view = notification();
    view.category = "im.received".to_string();
    view.attribution = NotificationAttribution::conflict(
        "Example Chat",
        "org.example.Chat",
        AttributionReason::ExecutableMismatch,
        "source /tmp/fake",
        "conflict:example-chat".to_string(),
    );
    view.actions.push(Action {
        key: "default".to_string(),
        label: "Open".to_string(),
    });

    let model = PopupEntryViewModel::for_notification_at(&view, 1_000);

    assert_eq!(model.kind, PopupKind::Communication);
    assert_eq!(
        model.secondary_claim.as_deref(),
        Some("Claimed app: Example Chat")
    );
    assert!(model.primary_actions.is_empty());
    assert!(model.overflow_actions.is_empty());
}
