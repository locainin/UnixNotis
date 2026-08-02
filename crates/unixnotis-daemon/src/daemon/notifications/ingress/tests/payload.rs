use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use zbus::zvariant::OwnedValue;

use super::{
    avatar_buffer_size_allowed, avatar_file_size_allowed, build_notification,
    materialize_sender_visual, may_read_sender_host_visual, owned_to_string, parse_actions,
    parse_urgency_hint, resolve_expiration, sanitize_hints_for_storage, sender_visual_role,
    string_to_owned_value, NotificationInput, SenderMetadata, SenderVisualRole, MAX_ACTIONS,
    MAX_BODY_BYTES, MAX_CONVERSATION_AVATAR_BYTES, MAX_SUMMARY_BYTES,
};
use unixnotis_core::{
    AttributionReason, Config, IdentityAssurance, InteractionPolicies, NotificationImage, Urgency,
};

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
        image_data: None,
        sender_visual: None,
        sender_visual_role: SenderVisualRole::ConversationAvatar,
        sender: SenderMetadata {
            sender_name: Some(":1.test".to_string()),
            sender_pid: Some(42),
            sender_start_time: Some(77),
            sender_executable: Some("/usr/bin/test-app".to_string()),
            sender_executable_identity: None,
            ..SenderMetadata::default()
        },
        attribution: unixnotis_core::NotificationAttribution::default(),
        attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
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
        image_data: None,
        sender_visual: None,
        sender_visual_role: SenderVisualRole::ConversationAvatar,
        sender: SenderMetadata {
            sender_name: Some(":1.test".to_string()),
            sender_pid: Some(42),
            sender_start_time: Some(77),
            sender_executable: Some("/usr/bin/test-app".to_string()),
            sender_executable_identity: None,
            ..SenderMetadata::default()
        },
        attribution: unixnotis_core::NotificationAttribution::default(),
        attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
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
        image_data: None,
        sender_visual: None,
        sender_visual_role: SenderVisualRole::ConversationAvatar,
        sender: SenderMetadata {
            sender_executable: Some("/usr/bin/messages".to_string()),
            ..SenderMetadata::default()
        },
        attribution: unixnotis_core::NotificationAttribution::verified(
            "Messages",
            "Messages",
            "org.example.Messages",
            "messages",
            AttributionReason::ExactSystemExecutable,
            "exact system executable /usr/bin/messages",
            "system-app:org.example.Messages".to_string(),
        ),
        attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
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
        image_data: None,
        sender_visual: None,
        sender_visual_role: SenderVisualRole::ConversationAvatar,
        sender: SenderMetadata {
            sender_name: Some(":1.hostile".to_string()),
            sender_executable: Some("/usr/bin/unknown-client".to_string()),
            ..SenderMetadata::default()
        },
        attribution: unixnotis_core::NotificationAttribution::conflict(
            "Password Manager",
            "org.example.PasswordManager",
            AttributionReason::ExecutableMismatch,
            "source /usr/bin/unknown-client",
            "executable:1:2".to_string(),
        ),
        attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
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
        view.attribution.status,
        unixnotis_core::AttributionStatus::Conflict
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
        image_data: None,
        sender_visual: None,
        sender_visual_role: SenderVisualRole::ConversationAvatar,
        sender: SenderMetadata::default(),
        attribution: unixnotis_core::NotificationAttribution::unresolved(
            "Messages",
            AttributionReason::MissingSenderEvidence,
            "",
            "unknown:messages".to_string(),
        ),
        attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
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
        view.attribution.status,
        unixnotis_core::AttributionStatus::Unresolved
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
        image_data: None,
        sender_visual: None,
        sender_visual_role: SenderVisualRole::ConversationAvatar,
        sender: SenderMetadata::default(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        expire_timeout: 0,
    });

    assert!(!notification.inline_reply.available);
    assert!(notification.inline_reply.placeholder.is_empty());
}

#[test]
fn conversation_avatar_never_changes_badge_or_unresolved_identity() {
    let avatar = unixnotis_core::ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![1, 2, 3, 255],
    };
    let notification = build_notification(NotificationInput {
        app_name: "Signal".to_string(),
        app_icon: "/tmp/contact.png".to_string(),
        summary: "New message".to_string(),
        body: "Hello".to_string(),
        actions: Vec::new(),
        hints: HashMap::new(),
        image_data: None,
        sender_visual: Some(avatar),
        sender_visual_role: SenderVisualRole::ConversationAvatar,
        sender: SenderMetadata::default(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        expire_timeout: 0,
    });

    assert_eq!(notification.attribution.display_name, "Unknown application");
    assert_eq!(
        notification.attribution.badge_icon,
        "application-x-executable-symbolic"
    );
    assert_eq!(
        notification.image.sender_visual_role,
        unixnotis_core::NotificationVisualRole::None
    );
}

#[test]
fn sender_image_path_is_not_retained_in_notification_model() {
    let mut hints = HashMap::new();
    hints.insert(
        "image-path".to_string(),
        string_to_owned_value("/tmp/message-image.png").expect("image path"),
    );

    let notification = build_notification(NotificationInput {
        app_name: "Messages".to_string(),
        app_icon: String::new(),
        summary: "New message".to_string(),
        body: "Hello".to_string(),
        actions: Vec::new(),
        hints,
        image_data: None,
        sender_visual: None,
        sender_visual_role: SenderVisualRole::ConversationAvatar,
        sender: SenderMetadata::default(),
        attribution: unixnotis_core::NotificationAttribution::verified(
            "Messages",
            "Messages",
            "org.example.Messages",
            "messages",
            AttributionReason::ExactSystemExecutable,
            "exact system executable",
            "verified:system-app:org.example.Messages".to_string(),
        ),
        attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        expire_timeout: 0,
    });

    assert!(notification.image.content_image.data.is_empty());
    assert!(!notification.hints.contains_key("image-path"));
}

#[test]
fn associated_sender_role_accepts_inline_reply_and_message_categories() {
    let attribution = unixnotis_core::NotificationAttribution::associated(
        "Messages",
        "Messages",
        "org.example.Messages",
        "messages",
        IdentityAssurance::SystemAssociated,
        InteractionPolicies::NATIVE_COMPATIBILITY,
        unixnotis_core::AttributionReason::ExactUserExecutable,
        "associated executable",
        "recognized:system-app:org.example.Messages:sender".to_string(),
    );
    let index = super::super::super::identity::DesktopIdentityIndex::default();

    assert_eq!(
        sender_visual_role(
            &attribution,
            &index,
            &HashMap::new(),
            &["inline-reply".to_string(), "Reply".to_string()],
            "",
        ),
        SenderVisualRole::ConversationAvatar
    );

    let mut hints = HashMap::new();
    hints.insert(
        "category".to_string(),
        string_to_owned_value("im.received").expect("category value"),
    );
    assert_eq!(
        sender_visual_role(&attribution, &index, &hints, &[], ""),
        SenderVisualRole::ConversationAvatar
    );

    let mut exact = HashMap::new();
    exact.insert(
        "category".to_string(),
        string_to_owned_value("im").expect("exact category value"),
    );
    assert_eq!(
        sender_visual_role(&attribution, &index, &exact, &[], ""),
        SenderVisualRole::ConversationAvatar
    );

    let mut unrelated = HashMap::new();
    unrelated.insert(
        "category".to_string(),
        string_to_owned_value("other").expect("unrelated category value"),
    );
    assert_eq!(
        sender_visual_role(&attribution, &index, &unrelated, &[], ""),
        SenderVisualRole::None
    );
    assert_eq!(
        sender_visual_role(&attribution, &index, &HashMap::new(), &[], ""),
        SenderVisualRole::None
    );
}

#[test]
fn associated_noncommunication_path_is_a_small_application_visual() {
    let attribution = unixnotis_core::NotificationAttribution::associated(
        "Example player",
        "Example player",
        "org.example.Player",
        "example-player",
        IdentityAssurance::SystemAssociated,
        InteractionPolicies::NATIVE_COMPATIBILITY,
        unixnotis_core::AttributionReason::ExactSystemExecutable,
        "associated executable",
        "associated:system-app:org.example.Player:sender".to_string(),
    );
    let role = sender_visual_role(
        &attribution,
        &super::super::super::identity::DesktopIdentityIndex::default(),
        &HashMap::new(),
        &[],
        "/tmp/application-icon.png",
    );

    assert_eq!(role, SenderVisualRole::ApplicationProvidedIcon);
}

#[test]
fn portal_association_cannot_start_host_avatar_materialization() {
    let attribution = unixnotis_core::NotificationAttribution::associated(
        "Portal app",
        "Portal app",
        "org.example.PortalApp",
        "portal-app",
        IdentityAssurance::PortalAssociated,
        InteractionPolicies::CONFIRM_ACTIONS,
        AttributionReason::PortalAppIdAssociation,
        "portal supplied app id",
        "recognized:portal:org.example.PortalApp".to_string(),
    );
    assert!(!may_read_sender_host_visual(&attribution));
    assert_eq!(
        sender_visual_role(
            &attribution,
            &super::super::super::identity::DesktopIdentityIndex::default(),
            &HashMap::new(),
            &["inline-reply".to_string(), "Reply".to_string()],
            "",
        ),
        SenderVisualRole::None
    );
}

#[test]
fn associated_noncommunication_icon_is_retained_as_a_decorative_visual() {
    let icon = unixnotis_core::ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![1, 2, 3, 255],
    };
    let notification = build_notification(NotificationInput {
        app_name: "Example player".to_string(),
        app_icon: "example-player".to_string(),
        summary: "Track".to_string(),
        body: "Artist".to_string(),
        actions: Vec::new(),
        hints: HashMap::new(),
        image_data: None,
        sender_visual: Some(icon),
        sender_visual_role: SenderVisualRole::ApplicationProvidedIcon,
        sender: SenderMetadata::default(),
        attribution: unixnotis_core::NotificationAttribution::associated(
            "Example player",
            "Example player",
            "org.example.Player",
            "example-player",
            IdentityAssurance::SystemAssociated,
            InteractionPolicies::NATIVE_COMPATIBILITY,
            AttributionReason::ExactSystemExecutable,
            "protected local association",
            "associated:system-app:org.example.Player".to_string(),
        ),
        attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        expire_timeout: 0,
    });

    assert_eq!(
        notification.image.sender_visual_role,
        unixnotis_core::NotificationVisualRole::ApplicationProvidedIcon
    );
    assert_eq!(notification.image.badge_icon, "example-player");
}

#[test]
fn large_avatar_is_downsampled_to_the_storage_bound() {
    let source = vec![255_u8; 256 * 128 * 4];
    let (width, height, data) = super::downsample_avatar(256, 128, source, 64).expect("downsample");
    assert_eq!((width, height), (64, 32));
    assert_eq!(data.len(), 64 * 32 * 4);
}

#[test]
fn avatar_downsampling_rejects_zero_dimensions_and_keeps_exact_size_images() {
    assert!(super::downsample_avatar(0, 1, Vec::new(), 64).is_none());
    assert!(super::downsample_avatar(1, 0, Vec::new(), 64).is_none());

    let source = vec![7_u8; 64 * 64 * 4];
    let source_ptr = source.as_ptr();
    let (width, height, data) = super::downsample_avatar(64, 64, source, 64).expect("exact bound");
    assert_eq!((width, height), (64, 64));
    assert_eq!(data.as_ptr(), source_ptr);
}

#[test]
fn avatar_downsampling_maps_horizontal_and_vertical_pixels_by_scale() {
    // Keep the source height unchanged after scaling so the early-return guard
    // must compare both dimensions rather than accepting one matching value
    let mut horizontal = vec![0_u8; 128 * 4];
    for x in 0..128 {
        horizontal[x * 4] = u8::try_from(x).expect("horizontal fixture value");
    }
    let (width, height, data) =
        super::downsample_avatar(128, 1, horizontal, 64).expect("horizontal downsample");
    assert_eq!((width, height), (64, 1));
    assert_eq!(data[4], 2);

    let mut vertical = vec![0_u8; 64 * 128 * 4];
    for y in 0..128 {
        vertical[y * 64 * 4] = u8::try_from(y).expect("vertical fixture value");
    }
    let (width, height, data) =
        super::downsample_avatar(64, 128, vertical, 64).expect("vertical downsample");
    assert_eq!((width, height), (32, 64));
    assert_eq!(data[32 * 4], 2);
}

#[cfg(target_os = "linux")]
#[test]
fn fifo_avatar_path_is_rejected_without_opening_a_blocking_reader() {
    let directory = std::env::temp_dir().join(format!(
        "unixnotis-avatar-fifo-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    std::fs::create_dir(&directory).expect("create temporary directory");
    let path = directory.join("avatar.fifo");
    let path_string = path.to_string_lossy().into_owned();
    let status = std::process::Command::new("mkfifo")
        .arg(&path)
        .status()
        .expect("mkfifo available");
    assert!(status.success());
    assert!(materialize_sender_visual(&path_string, 64).is_none());
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir(directory);
}

#[test]
fn absolute_avatar_path_is_materialized_into_bounded_raster_data() {
    // This is a tiny 1x1 RGBA PNG used only to exercise the real decoder
    let png = [
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8,
        0xcf, 0xc0, 0xf0, 0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("unixnotis-avatar-{suffix}.png"));
    std::fs::write(&path, png).expect("write avatar fixture");

    let avatar = materialize_sender_visual(path.to_str().expect("utf8 fixture path"), 64);
    let _ = std::fs::remove_file(&path);

    let avatar = avatar.expect("valid avatar should decode");
    assert_eq!((avatar.width, avatar.height), (1, 1));
    assert_eq!(avatar.channels, 4);
    assert_eq!(avatar.data.len(), 4);
}

#[test]
fn avatar_size_limits_accept_the_boundary_and_reject_one_byte_over() {
    assert!(avatar_file_size_allowed(MAX_CONVERSATION_AVATAR_BYTES));
    assert!(!avatar_file_size_allowed(MAX_CONVERSATION_AVATAR_BYTES + 1));
    assert!(avatar_buffer_size_allowed(
        MAX_CONVERSATION_AVATAR_BYTES as usize
    ));
    assert!(!avatar_buffer_size_allowed(
        MAX_CONVERSATION_AVATAR_BYTES as usize + 1
    ));
}

#[test]
fn relative_or_missing_avatar_path_is_rejected() {
    assert!(materialize_sender_visual("avatar.png", 64).is_none());
    assert!(materialize_sender_visual("/path/that/does/not/exist.png", 64).is_none());
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
        attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
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
        attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
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
