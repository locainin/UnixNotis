use super::ICON_UPDATE_QUEUE_CAPACITY;
use gtk::prelude::*;
use unixnotis_core::NotificationView;
use unixnotis_ui::presentation::{BadgePresentation, TrustLevel};

use super::super::cache::{image_key_matches, set_image_key, IconKey};

#[test]
fn icon_update_queue_capacity_remains_bounded() {
    assert_eq!(ICON_UPDATE_QUEUE_CAPACITY, 256);
}

#[gtk::test]
fn identity_badge_restores_semantic_fallback_visibility_on_a_recycled_image() {
    let resolver = super::IconResolver::new();
    let mut notification = NotificationView {
        id: 1,
        generation: 1,
        app_name: "Example".to_string(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: String::new(),
        body: String::new(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        urgency: 1,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 0,
        image: unixnotis_core::NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: 0,
    };
    // Remove every branding candidate so the semantic fallback path is exercised
    notification.attribution.badge_icon.clear();
    let image = gtk::Image::from_icon_name("folder");
    image.set_visible(false);

    resolver.apply_identity_badge(
        &image,
        &notification,
        BadgePresentation::UnknownApplication,
        TrustLevel::Unresolved,
        20,
        1,
    );

    assert!(image.get_visible());
    assert_eq!(
        image.icon_name().as_deref(),
        Some("unixnotis-app-unknown-symbolic")
    );
}

#[gtk::test]
fn clearing_identity_badge_invalidates_pending_icon_ownership() {
    let resolver = super::IconResolver::new();
    let image = gtk::Image::new();
    let old_key = IconKey::Name {
        name: "org.example.Old".to_string(),
        size: 20,
        scale: 1,
    };

    set_image_key(&image, old_key.clone());
    assert!(image_key_matches(&image, &old_key));

    resolver.clear_identity_badge(&image);

    assert!(!image_key_matches(&image, &old_key));
    assert!(image.paintable().is_none());
    assert!(!image.get_visible());
}
