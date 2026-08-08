use super::support::*;
use crate::store::notifications::rules::assurance_allows_app_rule;

#[test]
fn contains_ci_matches_ascii() {
    assert!(contains_ci("Example-Chat", "example"));
    assert!(contains_ci("example-chat", "Example"));
    assert!(!contains_ci("example-chat", "brave"));
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
            claimed_app: None,
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
    notification.attribution = unixnotis_core::NotificationAttribution::verified(
        "Test Application",
        "claimed",
        "org.example.Test",
        "test-app",
        unixnotis_core::AttributionReason::ExactSystemExecutable,
        "test identity",
        "verified:org.example.Test".to_string(),
    );
    notification.body = "body text".to_string();
    notification.category = Some("chat.message".to_string());
    notification.urgency = unixnotis_core::Urgency::Normal;
    notification.hints.insert(
        "urgency".to_string(),
        zbus::zvariant::OwnedValue::from(1_u32),
    );

    store.apply_rules(&mut notification);

    assert!(notification.suppress_popup);
    assert!(notification.suppress_sound);
    assert_eq!(notification.urgency, unixnotis_core::Urgency::Critical);
    assert_eq!(
        notification
            .hints
            .get("urgency")
            .and_then(|value| value.try_clone().ok())
            .and_then(|value| u32::try_from(value).ok()),
        Some(notification.urgency.as_u32())
    );
    assert_eq!(notification.expire_timeout, 1234);
    assert!(notification.is_resident);
    assert!(notification.is_transient);
}

#[test]
fn trusted_app_rule_does_not_match_spoofed_claim_or_escalate_urgency() {
    let config = Config {
        rules: vec![unixnotis_core::RuleConfig {
            app: Some("TrustedApp".to_string()),
            force_urgency: Some(unixnotis_core::RuleUrgency::Critical),
            ..unixnotis_core::RuleConfig::default()
        }],
        ..Config::default()
    };
    let store = NotificationStore::new(config);
    let mut notification = make_notification("spoofed claim");
    notification.app_name = "TrustedApp".to_string();
    notification.attribution = unixnotis_core::NotificationAttribution::verified(
        "Unrelated Application",
        "TrustedApp",
        "org.example.Unrelated",
        "unrelated",
        unixnotis_core::AttributionReason::ExactSystemExecutable,
        "test identity",
        "verified:org.example.Unrelated".to_string(),
    );

    store.apply_rules(&mut notification);

    assert_eq!(notification.urgency, unixnotis_core::Urgency::Normal);
}

#[test]
fn trusted_app_rule_matches_resolved_display_name_or_desktop_id() {
    let config = Config {
        rules: vec![unixnotis_core::RuleConfig {
            app: Some("TrustedApp".to_string()),
            no_popup: Some(true),
            ..unixnotis_core::RuleConfig::default()
        }],
        ..Config::default()
    };
    let store = NotificationStore::new(config);
    let mut notification = make_notification("trusted identity");
    notification.app_name = "Unrelated Claim".to_string();
    notification.attribution = unixnotis_core::NotificationAttribution::verified(
        "Trusted Application",
        "Unrelated Claim",
        "org.example.TrustedApp",
        "trusted-app",
        unixnotis_core::AttributionReason::ExactSystemExecutable,
        "test identity",
        "verified:org.example.TrustedApp".to_string(),
    );

    store.apply_rules(&mut notification);

    assert!(notification.suppress_popup);
}

#[test]
fn claimed_app_rule_intentionally_matches_sender_claim() {
    let config = Config {
        rules: vec![unixnotis_core::RuleConfig {
            claimed_app: Some("TrustedApp".to_string()),
            silent: Some(true),
            ..unixnotis_core::RuleConfig::default()
        }],
        ..Config::default()
    };
    let store = NotificationStore::new(config);
    let mut notification = make_notification("claimed identity");
    notification.app_name = "TrustedApp".to_string();

    store.apply_rules(&mut notification);

    assert!(notification.suppress_sound);
}

#[test]
fn trusted_app_rule_rejects_unresolved_and_conflicting_attribution() {
    let config = Config {
        rules: vec![unixnotis_core::RuleConfig {
            app: Some("TrustedApp".to_string()),
            no_popup: Some(true),
            ..unixnotis_core::RuleConfig::default()
        }],
        ..Config::default()
    };
    let store = NotificationStore::new(config);
    let mut unresolved = make_notification("unresolved");
    unresolved.app_name = "TrustedApp".to_string();
    unresolved.attribution = unixnotis_core::NotificationAttribution::unresolved(
        "TrustedApp",
        unixnotis_core::AttributionReason::MissingSenderEvidence,
        "test identity",
        "unknown:trusted-app".to_string(),
    );
    let mut conflict = make_notification("conflict");
    conflict.app_name = "TrustedApp".to_string();
    conflict.attribution = unixnotis_core::NotificationAttribution::conflict(
        "TrustedApp",
        "org.example.TrustedApp",
        unixnotis_core::AttributionReason::ExecutableMismatch,
        "test identity",
        "conflict:trusted-app".to_string(),
    );

    store.apply_rules(&mut unresolved);
    store.apply_rules(&mut conflict);

    assert!(!unresolved.suppress_popup);
    assert!(!conflict.suppress_popup);
}

#[test]
fn app_rules_accept_only_explicitly_resolved_assurance_levels() {
    use unixnotis_core::IdentityAssurance;

    for assurance in [
        IdentityAssurance::Authenticated,
        IdentityAssurance::SystemAssociated,
        IdentityAssurance::PortalAssociated,
        IdentityAssurance::UserAssociated,
    ] {
        assert!(assurance_allows_app_rule(assurance));
    }
    for assurance in [
        IdentityAssurance::Unresolved,
        IdentityAssurance::Conflict,
        IdentityAssurance::Relay,
    ] {
        assert!(!assurance_allows_app_rule(assurance));
    }
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
