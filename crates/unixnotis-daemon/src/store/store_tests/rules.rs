use super::*;

#[test]
fn contains_ci_matches_ascii() {
    assert!(contains_ci("Signal-Desktop", "signal"));
    assert!(contains_ci("signal-desktop", "Signal"));
    assert!(!contains_ci("signal-desktop", "brave"));
    assert!(contains_ci("mixedCase", "case"));
    assert!(contains_ci("mixedCase", ""));
    assert!(contains_ci("same", "same"));
    assert!(!contains_ci("short", "longer"));
}

#[test]
fn rules_require_all_filters_and_apply_every_mutation() {
    let config = Config {
        rules: vec![unixnotis_core::RuleConfig {
            name: Some("test-rule".to_string()),
            app: Some("test".to_string()),
            summary: Some("hello".to_string()),
            body: Some("body".to_string()),
            category: Some("chat".to_string()),
            urgency: Some(unixnotis_core::RuleUrgency::Normal),
            no_popup: Some(true),
            silent: Some(true),
            force_urgency: Some(unixnotis_core::RuleUrgency::Critical),
            expire_timeout_ms: Some(1234),
            resident: Some(true),
            transient: Some(true),
        }],
        ..Config::default()
    };
    let store = NotificationStore::new(config);
    let mut notification = make_notification("hello summary");
    notification.body = "body text".to_string();
    notification.category = Some("chat.message".to_string());
    notification.urgency = unixnotis_core::Urgency::Normal;

    store.apply_rules(&mut notification);

    assert!(notification.suppress_popup);
    assert!(notification.suppress_sound);
    assert_eq!(notification.urgency, unixnotis_core::Urgency::Critical);
    assert_eq!(notification.expire_timeout, 1234);
    assert!(notification.is_resident);
    assert!(notification.is_transient);
}

#[test]
fn rules_do_not_match_missing_category_or_wrong_urgency() {
    let config = Config {
        rules: vec![unixnotis_core::RuleConfig {
            category: Some("chat".to_string()),
            urgency: Some(unixnotis_core::RuleUrgency::Critical),
            no_popup: Some(true),
            ..unixnotis_core::RuleConfig::default()
        }],
        ..Config::default()
    };
    let store = NotificationStore::new(config);
    let mut notification = make_notification("hello");
    notification.urgency = unixnotis_core::Urgency::Normal;

    store.apply_rules(&mut notification);
    assert!(!notification.suppress_popup);

    notification.category = Some("chat".to_string());
    store.apply_rules(&mut notification);
    assert!(!notification.suppress_popup);
}

#[test]
fn rules_do_not_match_wrong_category_even_when_urgency_matches() {
    let config = Config {
        rules: vec![unixnotis_core::RuleConfig {
            category: Some("chat".to_string()),
            urgency: Some(unixnotis_core::RuleUrgency::Critical),
            no_popup: Some(true),
            ..unixnotis_core::RuleConfig::default()
        }],
        ..Config::default()
    };
    let store = NotificationStore::new(config);
    let mut notification = make_notification("hello");
    notification.category = Some("email".to_string());
    notification.urgency = unixnotis_core::Urgency::Critical;

    store.apply_rules(&mut notification);

    assert!(!notification.suppress_popup);
}
