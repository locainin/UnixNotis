use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    collect_icon_candidates, expand_rgb_to_rgba, resolve_icon_source, theme_path_uses_worker,
    worker_decodes_theme_path,
};
use unixnotis_core::{ImageData, NotificationImage, NotificationView};

fn notification_view(
    app_name: &str,
    attribution: unixnotis_core::NotificationAttribution,
    image: NotificationImage,
) -> NotificationView {
    NotificationView {
        id: 1,
        generation: 1,
        app_name: app_name.to_string(),
        attribution,
        summary: String::new(),
        body: String::new(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        urgency: 1,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 0,
        image,
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: 0,
    }
}

#[test]
fn badge_candidates_exclude_caller_content_icon() {
    let notification = notification_view(
        "sender-bin",
        unixnotis_core::NotificationAttribution {
            display_name: "Unknown application".to_string(),
            claimed_name: "Claimed Brand".to_string(),
            badge_icon: "sender-bin".to_string(),
            status: unixnotis_core::AttributionStatus::Conflict,
            reason: unixnotis_core::AttributionReason::ExecutableMismatch,
            diagnostic_detail: "sender executable mismatch".to_string(),
            group_key: "executable:1:2".to_string(),
            ..unixnotis_core::NotificationAttribution::default()
        },
        NotificationImage {
            badge_icon: "caller-content-icon".to_string(),
            ..NotificationImage::default()
        },
    );

    let candidates = collect_icon_candidates(&notification);

    assert!(candidates.iter().any(|candidate| candidate == "sender-bin"));
    assert!(!candidates
        .iter()
        .any(|candidate| candidate == "caller-content-icon"));
}

#[test]
fn badge_candidates_exclude_unresolved_application_claim() {
    let notification = notification_view(
        "Trusted Brand",
        unixnotis_core::NotificationAttribution {
            display_name: "Trusted Brand".to_string(),
            badge_icon: "dialog-warning-symbolic".to_string(),
            ..unixnotis_core::NotificationAttribution::default()
        },
        NotificationImage::default(),
    );

    let candidates = collect_icon_candidates(&notification);

    assert!(candidates
        .iter()
        .any(|candidate| candidate == "dialog-warning-symbolic"));
    assert!(!candidates
        .iter()
        .any(|candidate| candidate == "Trusted Brand"));
}

#[test]
fn unresolved_notifications_keep_only_bounded_decorative_theme_hints() {
    let attribution = unixnotis_core::NotificationAttribution {
        claimed_name: "Example Player".to_string(),
        ..unixnotis_core::NotificationAttribution::default()
    };
    let image = NotificationImage {
        claimed_theme_icon: "example-player".to_string(),
        ..NotificationImage::default()
    };

    let notification = notification_view("Unknown", attribution, image);
    let candidates = collect_icon_candidates(&notification);

    assert!(candidates
        .iter()
        .any(|candidate| candidate == "example-player"));
    assert!(candidates.iter().all(|candidate| !candidate.contains('/')));
}

#[test]
fn claimed_desktop_id_is_a_bounded_decorative_theme_hint() {
    let notification = notification_view(
        "Unknown",
        unixnotis_core::NotificationAttribution::default(),
        NotificationImage {
            claimed_desktop_id: "example-chat.desktop".to_string(),
            ..NotificationImage::default()
        },
    );

    let candidates = collect_icon_candidates(&notification);

    assert!(candidates
        .iter()
        .any(|candidate| candidate == "example-chat.desktop"));
    assert!(candidates
        .iter()
        .any(|candidate| candidate == "example-chat"));
    assert!(candidates.iter().all(|candidate| !candidate.contains('/')));
}

#[test]
fn unresolved_claimed_branding_precedes_the_generic_daemon_badge() {
    let notification = notification_view(
        "Example Application",
        unixnotis_core::NotificationAttribution::default(),
        NotificationImage {
            claimed_desktop_id: "org.example.App.desktop".to_string(),
            ..NotificationImage::default()
        },
    );
    let candidates = collect_icon_candidates(&notification);

    assert_eq!(
        candidates.first().map(String::as_str),
        Some("org.example.App.desktop")
    );
}

#[test]
fn associated_branding_still_precedes_presentation_claims() {
    let attribution = unixnotis_core::NotificationAttribution::associated(
        "Example Application",
        "Example Application",
        "org.example.Associated",
        "org.example.associated",
        unixnotis_core::IdentityAssurance::SystemAssociated,
        unixnotis_core::InteractionPolicies::NATIVE_COMPATIBILITY,
        unixnotis_core::AttributionReason::ExactSystemExecutable,
        "associated fixture",
        "associated:system-app:org.example.Associated".to_string(),
    );
    let notification = notification_view(
        "Example Application",
        attribution,
        NotificationImage {
            claimed_desktop_id: "org.example.Claimed.desktop".to_string(),
            ..NotificationImage::default()
        },
    );
    let candidates = collect_icon_candidates(&notification);

    assert_eq!(
        candidates.first().map(String::as_str),
        Some("org.example.associated")
    );
}

#[test]
fn icon_candidates_remove_duplicate_presentation_hints() {
    let notification = notification_view(
        "Example",
        unixnotis_core::NotificationAttribution {
            badge_icon: "folder".to_string(),
            desktop_id: "folder".to_string(),
            ..unixnotis_core::NotificationAttribution::default()
        },
        NotificationImage {
            claimed_theme_icon: "folder".to_string(),
            claimed_desktop_id: "folder".to_string(),
            ..NotificationImage::default()
        },
    );
    let candidates = collect_icon_candidates(&notification);
    let mut unique = candidates.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(candidates.len(), unique.len());
}

#[test]
fn expand_rgb_to_rgba_appends_alpha() {
    let data = ImageData {
        width: 2,
        height: 1,
        rowstride: 0,
        has_alpha: false,
        bits_per_sample: 8,
        channels: 3,
        data: vec![10, 20, 30, 40, 50, 60],
    };
    let (expanded, stride) = expand_rgb_to_rgba(&data).expect("rgb expansion");
    assert_eq!(stride, 8);
    assert_eq!(expanded, vec![10, 20, 30, 255, 40, 50, 60, 255]);
}

#[test]
fn theme_worker_accepts_only_its_bounded_raster_formats() {
    for path in ["icon.png", "icon.JPEG", "icon.webp", "icon.tiff"] {
        assert!(worker_decodes_theme_path(Path::new(path)), "{path}");
    }
    for path in ["icon.svg", "icon.svgz", "icon.xpm", "icon"] {
        assert!(!worker_decodes_theme_path(Path::new(path)), "{path}");
    }
}

#[cfg(unix)]
#[test]
fn theme_worker_path_requires_a_regular_non_symlink_raster_name() {
    use std::os::unix::fs::symlink;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "unixnotis-center-theme-path-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create theme path root");
    let raster = root.join("icon.png");
    let link = root.join("linked.png");
    let vector = root.join("icon.svg");
    fs::write(&raster, b"raster fixture").expect("write raster fixture");
    fs::write(&vector, b"vector fixture").expect("write vector fixture");
    symlink(&raster, &link).expect("create raster link");

    assert!(theme_path_uses_worker(&raster));
    assert!(!theme_path_uses_worker(&link));
    assert!(!theme_path_uses_worker(&vector));

    fs::remove_dir_all(root).expect("remove theme path root");
}

#[gtk::test]
fn standard_theme_icon_resolves_to_a_renderable_source() {
    let source = resolve_icon_source("folder", 24, 1)
        .or_else(|| resolve_icon_source("folder-symbolic", 24, 1));

    assert!(source.is_some());
}

#[test]
fn expand_rgb_to_rgba_honors_row_padding() {
    let data = ImageData {
        width: 2,
        height: 1,
        rowstride: 8,
        has_alpha: false,
        bits_per_sample: 8,
        channels: 3,
        data: vec![1, 2, 3, 4, 5, 6, 99, 100],
    };
    let (expanded, stride) = expand_rgb_to_rgba(&data).expect("rgb expansion");
    assert_eq!(stride, 8);
    assert_eq!(expanded, vec![1, 2, 3, 255, 4, 5, 6, 255]);
}
