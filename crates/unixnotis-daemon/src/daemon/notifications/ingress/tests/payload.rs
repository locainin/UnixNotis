use std::collections::HashMap;
use std::time::{Duration, Instant};

use zbus::zvariant::OwnedValue;

use super::{
    build_notification, owned_to_string, parse_actions, parse_urgency_hint, resolve_expiration,
    sanitize_hints_for_storage, string_to_owned_value, NotificationInput, SenderMetadata,
    MAX_ACTIONS, MAX_BODY_BYTES, MAX_SUMMARY_BYTES,
};
use unixnotis_core::{Config, NotificationImage, Urgency};

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
            sender_executable_identity: None,
            sender_cmdline: None,
        },
        attribution: unixnotis_core::NotificationAttribution::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
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
            sender_executable_identity: None,
            sender_cmdline: None,
        },
        attribution: unixnotis_core::NotificationAttribution::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        expire_timeout: 0,
    });

    assert_eq!(notification.app_name, "mailexe fake");
    assert_eq!(notification.summary, "safespoof");
    assert_eq!(notification.body, "line1\nline2tail");
    assert_eq!(notification.actions[0].label, "Open");
}

#[test]
fn build_notification_collects_inline_reply_action_and_kde_labels() {
    let mut hints = HashMap::<String, OwnedValue>::new();
    hints.insert(
        "x-kde-reply-placeholder-text".to_string(),
        string_to_owned_value("Write a reply").expect("placeholder value"),
    );
    hints.insert(
        "x-kde-reply-submit-button-text".to_string(),
        string_to_owned_value("Send now").expect("submit label value"),
    );
    hints.insert(
        "x-kde-reply-submit-button-icon-name".to_string(),
        string_to_owned_value("mail-send-symbolic").expect("submit icon value"),
    );

    let notification = build_notification(NotificationInput {
        app_name: "Messages".to_string(),
        app_icon: String::new(),
        summary: "New message".to_string(),
        body: "Are you coming?".to_string(),
        actions: vec!["inline-reply".to_string(), "Reply".to_string()],
        hints,
        sender: SenderMetadata {
            sender_executable: Some("/usr/bin/messages".to_string()),
            ..SenderMetadata::default()
        },
        attribution: unixnotis_core::NotificationAttribution::associated(
            "Messages",
            "org.example.Messages",
            "messages",
            "/usr/bin/messages",
            unixnotis_core::AttributionClass::SystemAssociated,
            false,
            "desktop:org.example.Messages".to_string(),
        ),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Allow,
        expire_timeout: 0,
    });

    assert!(notification.inline_reply.available);
    assert_eq!(notification.inline_reply.label, "Reply");
    assert_eq!(notification.inline_reply.placeholder, "Write a reply");
    assert_eq!(notification.inline_reply.submit_label, "Send now");
    assert_eq!(notification.inline_reply.submit_icon, "mail-send-symbolic");
}

#[test]
fn build_notification_keeps_protocol_reply_metadata_separate_from_denied_policy() {
    let notification = build_notification(NotificationInput {
        app_name: "Password Manager".to_string(),
        app_icon: "password-manager".to_string(),
        summary: "Sign in".to_string(),
        body: "Enter the account password".to_string(),
        actions: vec!["inline-reply".to_string(), "Password".to_string()],
        hints: HashMap::new(),
        sender: SenderMetadata {
            sender_name: Some(":1.hostile".to_string()),
            sender_executable: Some("/usr/bin/unknown-client".to_string()),
            ..SenderMetadata::default()
        },
        attribution: unixnotis_core::NotificationAttribution::conflict(
            "Password Manager",
            "source /usr/bin/unknown-client",
            "executable:1:2".to_string(),
        ),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        expire_timeout: 0,
    });

    assert!(notification.inline_reply.available);
    assert_eq!(
        notification.inline_reply_policy,
        unixnotis_core::InlineReplyPolicy::Deny
    );
    let view = notification.to_view();
    assert_eq!(view.app_name, "Unknown application");
    assert_eq!(
        view.attribution.class,
        unixnotis_core::AttributionClass::Conflict
    );
}

#[test]
fn build_notification_keeps_unknown_sender_reply_policy_denied() {
    let notification = build_notification(NotificationInput {
        app_name: "Messages".to_string(),
        app_icon: String::new(),
        summary: "New message".to_string(),
        body: String::new(),
        actions: vec!["inline-reply".to_string(), "Reply".to_string()],
        hints: HashMap::new(),
        sender: SenderMetadata::default(),
        attribution: unixnotis_core::NotificationAttribution::unknown(
            "Messages",
            "",
            "unknown:messages".to_string(),
        ),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        expire_timeout: 0,
    });

    assert!(notification.inline_reply.available);
    assert_eq!(
        notification.inline_reply_policy,
        unixnotis_core::InlineReplyPolicy::Deny
    );
    let view = notification.to_view();
    assert_eq!(view.app_name, "Messages");
    assert_eq!(
        view.attribution.class,
        unixnotis_core::AttributionClass::Unknown
    );
}

#[test]
fn build_notification_ignores_reply_hints_without_explicit_action() {
    let mut hints = HashMap::<String, OwnedValue>::new();
    hints.insert(
        "x-kde-reply-placeholder-text".to_string(),
        string_to_owned_value("Decoy reply").expect("placeholder value"),
    );

    let notification = build_notification(NotificationInput {
        app_name: "Messages".to_string(),
        app_icon: String::new(),
        summary: "New message".to_string(),
        body: String::new(),
        actions: vec!["default".to_string(), "Open".to_string()],
        hints,
        sender: SenderMetadata::default(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        expire_timeout: 0,
    });

    assert!(!notification.inline_reply.available);
    assert!(notification.inline_reply.placeholder.is_empty());
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
fn parse_actions_ignores_dangling_key_without_label() {
    let actions = parse_actions(vec![
        "default".to_string(),
        "Open".to_string(),
        "orphan-key".to_string(),
    ]);

    // D-Bus action arrays are pairs; a trailing key cannot produce a safe button
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].key, "default");
    assert_eq!(actions[0].label, "Open");
}

#[test]
fn parse_actions_reserves_capacity_for_complete_pairs_only() {
    let actions = parse_actions(vec![
        "default".to_string(),
        "Open".to_string(),
        "dismiss".to_string(),
        "Dismiss".to_string(),
    ]);

    assert_eq!(actions.len(), 2);
    assert_eq!(actions.capacity(), 2);
}

#[test]
fn sanitize_hints_drops_untrusted_and_bounds_strings() {
    let mut hints = HashMap::<String, OwnedValue>::new();
    hints.insert("transient".to_string(), OwnedValue::from(true));
    hints.insert("urgency".to_string(), OwnedValue::from(9u32));
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
    assert_eq!(sanitized.len(), 3);
    assert!(sanitized.contains_key("transient"));
    assert!(sanitized.contains_key("sound-name"));
    assert_eq!(
        u32::try_from(sanitized.get("urgency").expect("urgency")),
        Ok(2)
    );

    let sound_name = owned_to_string(
        sanitized
            .get("sound-name")
            .expect("sound-name should remain"),
    )
    .expect("sound-name should be string");
    assert!(sound_name.len() <= 2048);
}

#[test]
fn parse_urgency_hint_accepts_byte_and_integer_values_with_cap() {
    assert_eq!(parse_urgency_hint(&OwnedValue::from(0u8)), Some(0));
    assert_eq!(parse_urgency_hint(&OwnedValue::from(1u32)), Some(1));
    assert_eq!(parse_urgency_hint(&OwnedValue::from(99u32)), Some(2));
    assert_eq!(
        parse_urgency_hint(&string_to_owned_value("high").expect("string")),
        None
    );
}

#[test]
fn owned_to_string_accepts_only_string_values() {
    assert_eq!(
        owned_to_string(&string_to_owned_value("sound").expect("string")).as_deref(),
        Some("sound")
    );
    assert_eq!(owned_to_string(&OwnedValue::from(7u32)), None);
}

#[test]
fn resolve_expiration_respects_protocol_and_config_rules() {
    let mut config = Config::default();
    config.popups.default_timeout_ms = 5_000;
    config.popups.critical_timeout_ms = Some(9_000);

    let mut notification = unixnotis_core::Notification {
        id: 1,
        generation: 1,
        app_name: "app".to_string(),
        app_icon: String::new(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: "summary".to_string(),
        body: String::new(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        hints: HashMap::new(),
        urgency: Urgency::Normal,
        category: None,
        is_transient: false,
        is_resident: false,
        suppress_popup: false,
        suppress_sound: false,
        image: NotificationImage::default(),
        expire_timeout: -1,
        received_at: chrono::Utc::now(),
        sender_name: None,
        sender_pid: None,
        sender_start_time: None,
        sender_executable: None,
    };

    assert!(resolve_expiration(&config, &notification).is_some());

    notification.urgency = Urgency::Critical;
    assert!(resolve_expiration(&config, &notification).is_some());

    notification.expire_timeout = 0;
    assert!(resolve_expiration(&config, &notification).is_none());

    notification.expire_timeout = 100;
    notification.is_resident = true;
    assert!(resolve_expiration(&config, &notification).is_none());

    notification.is_resident = false;
    let before = Instant::now();
    let deadline = resolve_expiration(&config, &notification).expect("explicit timeout");
    assert!(deadline > before);
    assert!(deadline <= Instant::now() + Duration::from_millis(500));

    notification.expire_timeout = -1;
    notification.urgency = Urgency::Critical;
    config.popups.critical_timeout_ms = None;
    assert!(resolve_expiration(&config, &notification).is_none());
}

#[test]
fn resolve_expiration_treats_positive_timeout_as_caller_owned_even_when_default_is_zero() {
    let mut config = Config::default();
    config.popups.default_timeout_ms = 0;
    let mut notification = unixnotis_core::Notification {
        id: 1,
        generation: 1,
        app_name: "app".to_string(),
        app_icon: String::new(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        summary: "summary".to_string(),
        body: String::new(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        hints: HashMap::new(),
        urgency: Urgency::Normal,
        category: None,
        is_transient: false,
        is_resident: false,
        suppress_popup: false,
        suppress_sound: false,
        image: NotificationImage::default(),
        expire_timeout: 25,
        received_at: chrono::Utc::now(),
        sender_name: None,
        sender_pid: None,
        sender_start_time: None,
        sender_executable: None,
    };

    assert!(resolve_expiration(&config, &notification).is_some());

    notification.expire_timeout = -1;
    assert!(resolve_expiration(&config, &notification).is_none());
}
