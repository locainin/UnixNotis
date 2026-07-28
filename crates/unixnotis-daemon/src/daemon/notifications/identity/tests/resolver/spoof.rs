use super::*;

#[test]
fn user_shadow_cannot_join_the_system_desktop_group() {
    let system_identity = identity(30, 300, 0);
    let user_identity = identity(31, 310, 1000);
    let system = system_record(
        "org.signal.Signal",
        "Signal",
        "/usr/bin/signal-desktop",
        system_identity,
    );
    let mut user = DesktopRecord::fixture(
        "org.signal.Signal",
        "Signal",
        "/home/user/bin/signal",
        user_identity,
        false,
        false,
    );
    user.desktop_identity = Some(identity(32, 320, 1000));
    let index = DesktopIdentityIndex::from_records(vec![user, system], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Signal",
            desktop_entry: Some("org.signal.Signal"),
        },
        &sender("/home/user/bin/signal", user_identity),
        &index,
        &HashSet::new(),
    );

    assert_eq!(
        resolution.attribution.class,
        AttributionClass::UserAssociated
    );
    assert!(resolution.attribution.has_warning());
    assert!(resolution
        .attribution
        .group_key
        .starts_with("user-desktop:"));
    assert_ne!(
        resolution.attribution.group_key,
        "system-desktop:org.signal.Signal"
    );
}

#[test]
fn visually_confusable_system_brand_is_a_conflict() {
    let signal_identity = identity(40, 400, 0);
    let hostile_identity = identity(41, 410, 1000);
    let index = DesktopIdentityIndex::from_records(
        vec![system_record(
            "org.signal.Signal",
            "Signal",
            "/usr/bin/signal-desktop",
            signal_identity,
        )],
        Vec::new(),
    );

    for claim in ["Sіgnal", "Signaⅼ"] {
        let resolution = resolve_with_evidence(
            AppClaim {
                reported_name: claim,
                desktop_entry: None,
            },
            &sender("/tmp/fake", hostile_identity),
            &index,
            &HashSet::new(),
        );

        assert_eq!(resolution.attribution.class, AttributionClass::Conflict);
        assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    }
}

#[test]
fn basename_spoof_is_conflicting_and_cannot_join_or_reply_as_signal() {
    let signal_identity = identity(1, 10, 0);
    let hostile_identity = identity(7, 70, 1000);
    let index = DesktopIdentityIndex::from_records(
        vec![system_record(
            "org.signal.Signal",
            "Signal",
            "/usr/bin/signal-desktop",
            signal_identity,
        )],
        Vec::new(),
    );

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Signal",
            desktop_entry: None,
        },
        &sender("/tmp/signal-desktop", hostile_identity),
        &index,
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.class, AttributionClass::Conflict);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert_ne!(
        resolution.attribution.group_key,
        "desktop:org.signal.Signal"
    );
}

#[test]
fn exact_keepassxc_name_spoof_never_becomes_system_associated() {
    let keepass_identity = identity(2, 20, 0);
    let hostile_identity = identity(8, 80, 1000);
    let index = DesktopIdentityIndex::from_records(
        vec![system_record(
            "org.keepassxc.KeePassXC",
            "KeePassXC",
            "/usr/bin/keepassxc",
            keepass_identity,
        )],
        Vec::new(),
    );

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "KeePassXC",
            desktop_entry: None,
        },
        &sender("/tmp/keepassxc", hostile_identity),
        &index,
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.class, AttributionClass::Conflict);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn exact_system_notify_send_identity_is_a_non_replying_relay() {
    let relay_identity = identity(3, 30, 0);
    let index = DesktopIdentityIndex::from_records(
        Vec::new(),
        vec![(PathBuf::from("/usr/bin/notify-send"), relay_identity)],
    );

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Screenshot",
            desktop_entry: None,
        },
        &sender("/usr/bin/notify-send", relay_identity),
        &index,
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.class, AttributionClass::TrustedRelay);
    assert_eq!(resolution.attribution.display_name, "Screenshot");
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert!(!resolution.attribution.has_warning());
    assert!(!resolution.attribution.source_label.contains("unverified"));
}

#[test]
fn trusted_relay_claiming_a_system_app_keeps_the_relay_class_and_adds_a_warning() {
    let signal_identity = identity(1, 10, 0);
    let relay_identity = identity(3, 30, 0);
    let index = DesktopIdentityIndex::from_records(
        vec![system_record(
            "org.signal.Signal",
            "Signal",
            "/usr/bin/signal-desktop",
            signal_identity,
        )],
        vec![(PathBuf::from("/usr/bin/notify-send"), relay_identity)],
    );

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Signal",
            desktop_entry: None,
        },
        &sender("/usr/bin/notify-send", relay_identity),
        &index,
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.class, AttributionClass::TrustedRelay);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert!(resolution.attribution.has_warning());
    assert_ne!(
        resolution.attribution.group_key,
        "desktop:org.signal.Signal"
    );
}

#[test]
fn malicious_notify_send_basename_is_not_a_trusted_relay() {
    let real_relay = identity(3, 30, 0);
    let hostile_identity = identity(9, 90, 1000);
    let index = DesktopIdentityIndex::from_records(
        Vec::new(),
        vec![(PathBuf::from("/usr/bin/notify-send"), real_relay)],
    );

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Screenshot",
            desktop_entry: None,
        },
        &sender("/tmp/notify-send", hostile_identity),
        &index,
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.class, AttributionClass::Unknown);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn owned_dbus_application_name_without_executable_evidence_remains_unverified() {
    let app_identity = identity(4, 40, 0);
    let mut record = DesktopRecord::fixture(
        "org.example.App",
        "Example App",
        "/usr/bin/example-app",
        app_identity,
        true,
        true,
    );
    record.executable_identity = None;
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());
    let owned = HashSet::from(["org.example.app".to_string()]);

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Example App",
            desktop_entry: Some("org.example.App"),
        },
        &sender("/usr/lib/example-launcher", identity(5, 50, 0)),
        &index,
        &owned,
    );

    assert_eq!(resolution.attribution.class, AttributionClass::Unknown);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert!(resolution
        .attribution
        .source_label
        .contains("/usr/lib/example-launcher"));
}

#[test]
fn desktop_id_validation_never_accepts_a_path_or_control_character() {
    assert_eq!(
        validate_desktop_id("org.signal.Signal.desktop").as_deref(),
        Some("org.signal.Signal")
    );
    assert_eq!(validate_desktop_id("../signal"), None);
    assert_eq!(validate_desktop_id("org.example.\nApp"), None);
    assert_eq!(validate_desktop_id("."), None);
    assert_eq!(validate_desktop_id(".desktop"), None);
    assert_eq!(
        validate_desktop_id(&"a".repeat(256)).map(|id| id.len()),
        Some(256)
    );
    assert_eq!(validate_desktop_id(&"a".repeat(257)), None);
}
