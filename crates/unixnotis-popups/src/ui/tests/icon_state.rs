use super::*;

#[test]
fn negative_icon_cache_expires_at_the_ttl_boundary() {
    let now = Instant::now();

    let fresh = now
        .checked_sub(Duration::from_secs(14))
        .expect("fresh timestamp should remain representable");
    let expired = now
        .checked_sub(NEGATIVE_ICON_CACHE_TTL)
        .expect("expired timestamp should remain representable");

    assert!(negative_cache_is_fresh(fresh, now));
    assert!(!negative_cache_is_fresh(expired, now));
}

#[test]
fn negative_icon_cache_handles_future_timestamp_without_panicking() {
    let now = Instant::now();

    assert!(negative_cache_is_fresh(now + Duration::from_secs(1), now));
}

#[test]
fn small_square_image_data_is_suppressed_as_decorative_content() {
    let mut notification = notification_with_image();
    notification.image.has_image_data = true;
    notification.image.image_data.width = 96;
    notification.image.image_data.height = 96;

    assert!(content_image_is_decorative(&notification));
}

#[test]
fn media_category_keeps_a_small_square_content_thumbnail() {
    for category in ["image.photo", "media.video", "photo"] {
        let mut notification = notification_with_image();
        notification.category = category.to_string();
        notification.image.has_image_data = true;
        notification.image.image_data.width = 96;
        notification.image.image_data.height = 96;

        assert!(
            !content_image_is_decorative(&notification),
            "{category} should preserve real content"
        );
    }
}

#[test]
fn content_source_matching_badge_is_suppressed_without_image_data() {
    let mut notification = notification_with_image();
    notification.attribution.badge_icon = "signal".to_string();
    notification.image.icon_name = "signal".to_string();

    assert!(content_image_is_decorative(&notification));
}

#[test]
fn content_path_matching_badge_is_suppressed_without_image_data() {
    let mut notification = notification_with_image();
    notification.attribution.badge_icon = "/usr/share/icons/signal.png".to_string();
    notification.image.image_path = "/usr/share/icons/signal.png".to_string();

    assert!(content_image_is_decorative(&notification));
}

#[test]
fn empty_badge_does_not_match_empty_content_sources() {
    let notification = notification_with_image();

    assert!(!content_image_is_decorative(&notification));
}

#[test]
fn square_image_heuristic_requires_data_positive_dimensions_and_size_limit() {
    let cases = [
        (false, 96, 96),
        (true, 0, 0),
        (true, 96, 72),
        (true, 129, 129),
    ];

    for (has_image_data, width, height) in cases {
        let mut notification = notification_with_image();
        notification.image.has_image_data = has_image_data;
        notification.image.image_data.width = width;
        notification.image.image_data.height = height;

        assert!(
            !content_image_is_decorative(&notification),
            "data={has_image_data} width={width} height={height} should remain nondecorative"
        );
    }
}

fn notification_with_image() -> unixnotis_core::NotificationView {
    unixnotis_core::NotificationView {
        id: 1,
        generation: 1,
        app_name: "Example".to_string(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: "Summary".to_string(),
        body: "Body".to_string(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        urgency: 1,
        category: String::new(),
        is_transient: false,
        image: unixnotis_core::NotificationImage::default(),
    }
}
