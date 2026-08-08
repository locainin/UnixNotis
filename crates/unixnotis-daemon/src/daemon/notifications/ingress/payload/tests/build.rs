use super::*;

#[test]
fn retained_urgency_hint_always_matches_canonical_notification_urgency() {
    for raw in 0..=u8::MAX {
        let notification = build_notification(NotificationInput {
            app_name: "app".to_string(),
            app_icon: String::new(),
            summary: "summary".to_string(),
            body: String::new(),
            actions: Vec::new(),
            hints: HashMap::from([("urgency".to_string(), OwnedValue::from(raw))]),
            image_data: None,
            sender_visual_data: None,
            sender_visual: None,
            sender_visual_role: SenderVisualRole::None,
            sender: SenderMetadata::default(),
            attribution: unixnotis_core::NotificationAttribution::default(),
            attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
            inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
            expire_timeout: 0,
        });
        let stored = u32::try_from(
            notification
                .hints
                .get("urgency")
                .expect("canonical urgency hint should be retained"),
        )
        .expect("canonical urgency hint should use an unsigned integer");

        assert_eq!(
            stored,
            notification.urgency.as_u32(),
            "raw urgency {raw} must not diverge from the canonical field"
        );
    }
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
        image_data: None,
        sender_visual_data: None,
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
fn build_notification_rejects_content_pixels_above_retained_limit() {
    let notification = build_notification(NotificationInput {
        app_name: "Example viewer".to_string(),
        app_icon: "example-viewer".to_string(),
        summary: "Image".to_string(),
        body: "Attachment".to_string(),
        actions: Vec::new(),
        hints: HashMap::<String, OwnedValue>::new(),
        image_data: Some(unixnotis_core::ImageData {
            width: 512,
            height: 512,
            rowstride: 512 * 4,
            has_alpha: true,
            bits_per_sample: 8,
            channels: 4,
            data: vec![0; 512 * 512 * 4],
        }),
        sender_visual_data: None,
        sender_visual: None,
        sender_visual_role: SenderVisualRole::None,
        sender: SenderMetadata::default(),
        attribution: unixnotis_core::NotificationAttribution::default(),
        attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        expire_timeout: 0,
    });

    assert!(notification.image.content_image.data.is_empty());
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
        sender_visual_data: None,
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
        sender_visual_data: None,
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
        sender_visual_data: None,
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
        sender_visual_data: None,
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
        sender_visual_data: None,
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
        app_name: "Example Chat".to_string(),
        app_icon: "/tmp/contact.png".to_string(),
        summary: "New message".to_string(),
        body: "Hello".to_string(),
        actions: Vec::new(),
        hints: HashMap::new(),
        image_data: None,
        sender_visual_data: None,
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
        sender_visual_data: None,
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
fn associated_communication_image_data_becomes_a_bounded_conversation_avatar() {
    let image = unixnotis_core::ImageData {
        width: 128,
        height: 128,
        rowstride: 128 * 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![7; 128 * 128 * 4],
    };
    let notification = build_notification(NotificationInput {
        app_name: "Messages".to_string(),
        app_icon: String::new(),
        summary: "New message".to_string(),
        body: "Hello".to_string(),
        actions: Vec::new(),
        hints: HashMap::from([(
            "category".to_string(),
            string_to_owned_value("im.received").expect("category"),
        )]),
        image_data: Some(image),
        sender_visual_data: None,
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

    assert_eq!(
        notification.image.sender_visual_role,
        unixnotis_core::NotificationVisualRole::ConversationAvatar
    );
    assert!(notification.image.content_image.data.is_empty());
    assert_eq!(notification.image.sender_visual.width, 64);
    assert_eq!(notification.image.sender_visual.height, 64);
    assert!(notification.image.sender_visual.data.len() <= 64 * 64 * 4);

    // The production view keeps the bounded avatar role and leaves message content empty
    let view = notification.to_view();
    assert_eq!(
        view.image.sender_visual_role,
        unixnotis_core::NotificationVisualRole::ConversationAvatar
    );
    assert!(!view.image.sender_visual.data.is_empty());
    assert!(view.image.content_image.data.is_empty());
}

#[test]
fn unassociated_communication_image_data_stays_untrusted_content() {
    let image = unixnotis_core::ImageData {
        width: 1,
        height: 1,
        rowstride: 4,
        has_alpha: true,
        bits_per_sample: 8,
        channels: 4,
        data: vec![7, 8, 9, 255],
    };
    let notification = build_notification(NotificationInput {
        app_name: "Messages".to_string(),
        app_icon: String::new(),
        summary: "New message".to_string(),
        body: "Hello".to_string(),
        actions: Vec::new(),
        hints: HashMap::new(),
        image_data: Some(image),
        sender_visual_data: None,
        sender_visual: None,
        sender_visual_role: SenderVisualRole::None,
        sender: SenderMetadata::default(),
        attribution: unixnotis_core::NotificationAttribution::unresolved(
            "Messages",
            AttributionReason::MissingSenderEvidence,
            "no sender evidence",
            "claim:messages".to_string(),
        ),
        attribution_diagnostics: unixnotis_core::AttributionDiagnostics::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        expire_timeout: 0,
    });

    assert_eq!(
        notification.image.sender_visual_role,
        unixnotis_core::NotificationVisualRole::None
    );
    assert!(!notification.image.content_image.data.is_empty());
    assert!(notification.image.sender_visual.data.is_empty());
}
