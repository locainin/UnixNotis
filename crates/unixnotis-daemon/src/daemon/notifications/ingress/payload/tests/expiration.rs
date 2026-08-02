use super::*;
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
