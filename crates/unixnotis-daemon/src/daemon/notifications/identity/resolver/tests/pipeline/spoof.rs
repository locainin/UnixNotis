//! Spoofing and conflicting-identity regressions

use super::super::*;

#[test]
fn sender_metadata_timeout_is_recognized_not_conflict() {
    let protected_identity = identity(39, 390, 0);
    let index = DesktopIdentityIndex::from_records(
        vec![system_record(
            "org.example.Protected",
            "Protected",
            "/usr/bin/protected",
            protected_identity,
        )],
        Vec::new(),
    );

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Protected",
            desktop_entry: None,
        },
        &SenderMetadata::default(),
        &index,
    );

    assert_eq!(
        resolution.attribution.status,
        AttributionStatus::Recognized,
        "a timed-out sender lookup cannot prove impersonation"
    );
}

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
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Recognized);
    assert_ne!(resolution.attribution.status, AttributionStatus::Conflict);
    assert!(resolution
        .attribution
        .group_key
        .starts_with("recognized:user-app:"));
    assert_ne!(
        resolution.attribution.group_key,
        "verified:system-app:org.signal.Signal"
    );
}

#[test]
fn user_desktop_mismatch_cannot_manufacture_a_conflict() {
    let user_identity = identity(34, 340, 1_000);
    let hostile_identity = identity(35, 350, 1_000);
    let mut user = DesktopRecord::fixture(
        "org.example.Local",
        "Local App",
        "/home/user/bin/local-app",
        user_identity,
        false,
    );
    user.desktop_identity = Some(identity(36, 360, 1_000));
    let index = DesktopIdentityIndex::from_records(vec![user], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Local App",
            desktop_entry: Some("org.example.Local"),
        },
        &sender("/tmp/unrelated", hostile_identity),
        &index,
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Recognized);
    assert_ne!(resolution.attribution.status, AttributionStatus::Conflict);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn protected_conflict_evidence_outranks_a_user_desktop_shadow() {
    let protected_identity = identity(37, 370, 0);
    let user_identity = identity(38, 380, 1_000);
    let hostile_identity = identity(39, 390, 0);
    let protected = system_record(
        "org.example.Protected",
        "Protected App",
        "/usr/bin/protected-app",
        protected_identity,
    );
    let mut user = DesktopRecord::fixture(
        "org.example.Protected.Handler",
        "Protected App",
        "/home/user/bin/protected-handler",
        user_identity,
        false,
    );
    user.desktop_identity = Some(identity(40, 400, 1_000));
    let unrelated = system_record(
        "org.example.Unrelated",
        "Unrelated App",
        "/usr/bin/unrelated",
        hostile_identity,
    );
    let index = DesktopIdentityIndex::from_records(vec![user, protected, unrelated], Vec::new());
    let different = sender("/usr/bin/unrelated", hostile_identity);

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Protected App",
            desktop_entry: Some("org.example.Protected.Handler"),
        },
        &different,
        &index,
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Conflict);
    assert_eq!(resolution.attribution.desktop_id, "org.example.Protected");
    assert_eq!(resolution.diagnostics.record_trust, RecordTrust::System);
}

#[test]
fn ambiguous_protected_records_are_unresolved_not_conflicting() {
    let first = system_record(
        "org.example.First",
        "Shared Label",
        "/usr/bin/first-app",
        identity(41, 410, 0),
    );
    let second = system_record(
        "org.example.Second",
        "Shared Label",
        "/usr/bin/second-app",
        identity(42, 420, 0),
    );
    let index = DesktopIdentityIndex::from_records(vec![first, second], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Shared Label",
            desktop_entry: None,
        },
        &sender("/usr/bin/unrelated", identity(43, 430, 0)),
        &index,
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Unresolved);
    assert_eq!(
        resolution.attribution.reason,
        unixnotis_core::AttributionReason::AmbiguousDesktopRecords
    );
    assert_ne!(resolution.attribution.status, AttributionStatus::Conflict);
}

#[test]
fn visually_confusable_system_brand_without_contradictory_owner_is_recognized() {
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
        );

        assert_eq!(resolution.attribution.status, AttributionStatus::Recognized);
        assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    }
}

#[test]
fn basename_spoof_without_immutable_owner_is_recognized_without_actions() {
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
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Recognized);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert_eq!(
        resolution.diagnostics.verification,
        LaunchVerificationView::InsufficientEvidence
    );
    assert_ne!(
        resolution.attribution.group_key,
        "desktop:org.signal.Signal"
    );
}

#[test]
fn exact_protected_name_without_contradictory_owner_stays_recognized() {
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
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Recognized);
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
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Relay);
    assert_eq!(
        resolution.attribution.display_name,
        "Command-line notification"
    );
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert!(!resolution
        .attribution
        .diagnostic_detail
        .contains("unverified"));
}

#[test]
fn trusted_relay_claiming_a_system_app_stays_relay_without_conflict() {
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
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Relay);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert_ne!(resolution.attribution.status, AttributionStatus::Conflict);
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
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Unresolved);
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
    );
    record.executable_identity = None;
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Example App",
            desktop_entry: Some("org.example.App"),
        },
        &sender("/usr/lib/example-launcher", identity(5, 50, 0)),
        &index,
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Recognized);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert!(resolution
        .attribution
        .diagnostic_detail
        .contains("/usr/lib/example-launcher"));
}
