use std::collections::HashMap;

use zbus::zvariant::OwnedValue;

use super::{
    build_notification, display_width, normalize_text_for_layout, owned_to_string, parse_actions,
    sanitize_hints_for_storage, string_to_owned_value, truncate_utf8_bytes, NotificationInput,
    SenderMetadata, MAX_ACTIONS, MAX_BODY_BYTES, MAX_SUMMARY_BYTES,
};

#[test]
fn truncate_utf8_bytes_preserves_character_boundaries() {
    let value = "abc🙂def";
    let truncated = truncate_utf8_bytes(value, 5);
    assert_eq!(truncated, "abc");
}

#[test]
fn build_notification_clamps_summary_and_body_sizes() {
    let summary = "S".repeat(MAX_SUMMARY_BYTES + 128);
    let body = "B".repeat(MAX_BODY_BYTES + 512);

    let notification = build_notification(NotificationInput {
        app_name: "app".to_string(),
        app_icon: "icon".to_string(),
        summary,
        body,
        actions: Vec::new(),
        hints: HashMap::<String, OwnedValue>::new(),
        sender: SenderMetadata {
            sender_name: Some(":1.test".to_string()),
            sender_pid: Some(42),
            sender_start_time: Some(77),
            sender_executable: Some("/usr/bin/test-app".to_string()),
        },
        expire_timeout: 0,
    });

    assert!(notification.summary.len() <= MAX_SUMMARY_BYTES);
    assert!(notification.body.len() <= MAX_BODY_BYTES);
}

#[test]
fn build_notification_strips_display_spoofing_controls() {
    let notification = build_notification(NotificationInput {
        app_name: "mail\u{202E}exe\nfake".to_string(),
        app_icon: "icon".to_string(),
        summary: "safe\u{202E}spoof".to_string(),
        body: "line1\nline2\u{2066}tail".to_string(),
        actions: vec!["default".to_string(), "Open\u{202E}".to_string()],
        hints: HashMap::<String, OwnedValue>::new(),
        sender: SenderMetadata {
            sender_name: Some(":1.test".to_string()),
            sender_pid: Some(42),
            sender_start_time: Some(77),
            sender_executable: Some("/usr/bin/test-app".to_string()),
        },
        expire_timeout: 0,
    });

    assert_eq!(notification.app_name, "mailexe fake");
    assert_eq!(notification.summary, "safespoof");
    assert_eq!(notification.body, "line1\nline2tail");
    assert_eq!(notification.actions[0].label, "Open");
}

#[test]
fn parse_actions_caps_pairs() {
    let mut raw = Vec::new();
    for idx in 0..(MAX_ACTIONS + 10) {
        raw.push(format!("key-{idx}"));
        raw.push(format!("label-{idx}"));
    }

    let actions = parse_actions(raw);
    assert_eq!(actions.len(), MAX_ACTIONS);
}

#[test]
fn sanitize_hints_drops_untrusted_and_bounds_strings() {
    let mut hints = HashMap::<String, OwnedValue>::new();
    hints.insert("transient".to_string(), OwnedValue::from(true));
    hints.insert(
        "sound-name".to_string(),
        string_to_owned_value(&"n".repeat(5000)).expect("sound-name"),
    );
    hints.insert("image-data".to_string(), OwnedValue::from(123u32));
    hints.insert(
        "x-custom".to_string(),
        string_to_owned_value("custom").expect("custom"),
    );

    let sanitized = sanitize_hints_for_storage(hints);
    assert_eq!(sanitized.len(), 2);
    assert!(sanitized.contains_key("transient"));
    assert!(sanitized.contains_key("sound-name"));

    let sound_name = owned_to_string(
        sanitized
            .get("sound-name")
            .expect("sound-name should remain"),
    )
    .expect("sound-name should be string");
    assert!(sound_name.len() <= 2048);
}

#[test]
fn normalize_text_for_layout_folds_long_unbroken_tokens() {
    let input = "x".repeat(200);
    let normalized = normalize_text_for_layout(&input, 96);
    assert!(normalized.contains('…'));
    let longest = normalized
        .split_whitespace()
        .map(|part| part.chars().filter(|ch| ch.is_ascii_alphanumeric()).count())
        .max()
        .unwrap_or(0);
    assert!(longest <= 96);
}

#[test]
fn normalize_text_for_layout_keeps_char_count_bound_with_ellipsis() {
    let input = "x".repeat(200);
    let normalized = normalize_text_for_layout(&input, 96);
    assert!(normalized.contains('…'));
    assert!(normalized.chars().count() <= 96);
}

#[test]
fn normalize_text_for_layout_limits_wide_glyph_runs() {
    let input = "界".repeat(120);
    let normalized = normalize_text_for_layout(&input, 96);
    let width: usize = normalized.chars().map(display_width).sum();
    assert!(width <= 96);
}

#[test]
fn normalize_text_for_layout_limits_emoji_joiner_runs() {
    let input = "👨\u{200D}👩\u{200D}👧\u{200D}👦".repeat(80);
    let normalized = normalize_text_for_layout(&input, 96);
    let width: usize = normalized.chars().map(display_width).sum();
    assert!(width <= 96);
}
