use super::{
    ApplicationActionPolicy, AttributionReason, AttributionStatus, InlineReplyPolicy,
    NotificationAttribution,
};
use zbus::zvariant::{serialized::Context, to_bytes, Type, LE};

#[test]
fn attribution_wire_enums_use_declared_one_byte_values() {
    let context = Context::new_dbus(LE, 0);

    for (status, discriminant) in [
        (AttributionStatus::Verified, 0_u8),
        (AttributionStatus::Recognized, 1),
        (AttributionStatus::Unresolved, 2),
        (AttributionStatus::Conflict, 3),
        (AttributionStatus::Relay, 4),
    ] {
        let encoded = to_bytes(context, &status).expect("serialize attribution status");
        assert_eq!(AttributionStatus::signature(), u8::signature());
        assert_eq!(encoded.bytes(), &[discriminant]);
        let decoded: AttributionStatus = encoded
            .deserialize()
            .expect("deserialize attribution status")
            .0;
        assert_eq!(decoded, status);
    }

    for (reason, discriminant) in [
        (AttributionReason::ExactSystemExecutable, 0_u8),
        (AttributionReason::VerifiedPortalAppId, 1),
        (AttributionReason::ExactUserExecutable, 2),
        (AttributionReason::VerifiedProtectedPayload, 3),
        (AttributionReason::TrustedRelayExecutable, 4),
        (AttributionReason::MissingSenderEvidence, 10),
        (AttributionReason::MissingCommandLine, 11),
        (AttributionReason::AmbiguousDesktopRecords, 12),
        (AttributionReason::DynamicLaunchContract, 13),
        (AttributionReason::UnsupportedWrapper, 14),
        (AttributionReason::NoDesktopCandidate, 15),
        (AttributionReason::ExecutableMismatch, 20),
        (AttributionReason::ProtectedPayloadMismatch, 21),
        (AttributionReason::ApplicationClaimMismatch, 22),
    ] {
        let encoded = to_bytes(context, &reason).expect("serialize attribution reason");
        assert_eq!(AttributionReason::signature(), u8::signature());
        assert_eq!(encoded.bytes(), &[discriminant]);
        let decoded: AttributionReason = encoded
            .deserialize()
            .expect("deserialize attribution reason")
            .0;
        assert_eq!(decoded, reason);
    }

    for (policy, discriminant) in [
        (InlineReplyPolicy::Allow, 0_u8),
        (InlineReplyPolicy::Deny, 2),
    ] {
        let encoded = to_bytes(context, &policy).expect("serialize inline reply policy");
        assert_eq!(InlineReplyPolicy::signature(), u8::signature());
        assert_eq!(encoded.bytes(), &[discriminant]);
    }
}

#[test]
fn attribution_wire_enums_reject_unknown_discriminants() {
    let context = Context::new_dbus(LE, 0);
    let unknown = to_bytes(context, &u8::MAX).expect("serialize unknown byte");

    assert!(unknown.deserialize::<AttributionStatus>().is_err());
    assert!(unknown.deserialize::<AttributionReason>().is_err());

    let unused_policy = to_bytes(context, &1_u8).expect("serialize unused policy byte");
    assert!(unused_policy.deserialize::<InlineReplyPolicy>().is_err());
}

#[test]
fn verified_identity_keeps_claim_and_diagnostics_structured() {
    let attribution = NotificationAttribution::verified(
        "Example Chat",
        "Example Chat",
        "org.example.Chat",
        "org.example.Chat",
        AttributionReason::ExactSystemExecutable,
        "/usr/bin/example-chat",
        "system-app:org.example.Chat".to_string(),
    );

    assert_eq!(attribution.display_name, "Example Chat");
    assert_eq!(attribution.claimed_name, "Example Chat");
    assert_eq!(attribution.status, AttributionStatus::Verified);
    assert_eq!(attribution.reason, AttributionReason::ExactSystemExecutable);
    assert_eq!(
        attribution.group_key, "system-app:org.example.Chat",
        "a valid daemon group key must survive wire construction"
    );
}

#[test]
fn recognized_identity_preserves_canonical_application_fields() {
    let attribution = NotificationAttribution::recognized(
        "Example Chat",
        "Caller label",
        "org.example.Chat",
        "org.example.Chat",
        AttributionReason::MissingCommandLine,
        "the sender command line was unavailable",
        "recognized:system-app:org.example.Chat:7:11".to_string(),
    );

    assert_eq!(attribution.display_name, "Example Chat");
    assert_eq!(attribution.claimed_name, "Caller label");
    assert_eq!(attribution.desktop_id, "org.example.Chat");
    assert_eq!(attribution.badge_icon, "org.example.Chat");
    assert_eq!(attribution.status, AttributionStatus::Recognized);
    assert_eq!(attribution.reason, AttributionReason::MissingCommandLine);
    assert_eq!(
        attribution.group_key,
        "recognized:system-app:org.example.Chat:7:11"
    );
}

#[test]
fn unresolved_identity_preserves_claim_reason_and_isolated_group() {
    let attribution = NotificationAttribution::unresolved(
        "Caller label",
        AttributionReason::NoDesktopCandidate,
        "no desktop candidate matched",
        "unresolved:7:11:callerlabel".to_string(),
    );

    assert_eq!(attribution.display_name, "Unknown application");
    assert_eq!(attribution.claimed_name, "Caller label");
    assert!(attribution.desktop_id.is_empty());
    assert_eq!(attribution.badge_icon, "application-x-executable-symbolic");
    assert_eq!(attribution.status, AttributionStatus::Unresolved);
    assert_eq!(attribution.reason, AttributionReason::NoDesktopCandidate);
    assert_eq!(attribution.group_key, "unresolved:7:11:callerlabel");
}

#[test]
fn empty_group_key_fails_closed_to_unknown() {
    let attribution = NotificationAttribution::unresolved(
        "Caller label",
        AttributionReason::MissingSenderEvidence,
        "",
        " \n\t ".to_string(),
    );

    assert_eq!(
        attribution.group_key, "unknown",
        "empty or display-control-only group keys must not escape construction"
    );
}

#[test]
fn conflict_keeps_claim_out_of_human_diagnostic_state() {
    let attribution = NotificationAttribution::conflict(
        "Password Manager",
        "org.example.PasswordManager",
        AttributionReason::ExecutableMismatch,
        "sender executable differs from the protected record",
        "unknown:7:9:passwordmanager".to_string(),
    );

    assert_eq!(attribution.display_name, "Unknown application");
    assert_eq!(attribution.claimed_name, "Password Manager");
    assert_eq!(attribution.status, AttributionStatus::Conflict);
    assert!(!attribution.diagnostic_detail.contains("Claims to be"));
}

#[test]
fn relay_never_promotes_the_caller_label_to_primary_identity() {
    let attribution = NotificationAttribution::relay(
        "Example Chat",
        "Sent via /usr/bin/notify-send",
        "relay:1:2:examplechat".to_string(),
    );

    assert_eq!(attribution.display_name, "Command-line notification");
    assert_eq!(attribution.claimed_name, "Example Chat");
    assert_eq!(attribution.status, AttributionStatus::Relay);
}

#[test]
fn only_verified_identity_allows_application_actions() {
    let verified = NotificationAttribution::verified(
        "Verified",
        "Verified",
        "org.example.Verified",
        "verified",
        AttributionReason::ExactSystemExecutable,
        "",
        "system-app:verified".to_string(),
    );
    assert_eq!(
        verified.application_action_policy(),
        ApplicationActionPolicy::Allow
    );

    for attribution in [
        NotificationAttribution::recognized(
            "Local",
            "Local",
            "org.example.Local",
            "local",
            AttributionReason::ExactUserExecutable,
            "",
            "user-app:local".to_string(),
        ),
        NotificationAttribution::unresolved(
            "Unknown",
            AttributionReason::MissingSenderEvidence,
            "",
            "unknown:unknown".to_string(),
        ),
        NotificationAttribution::conflict(
            "Conflict",
            "org.example.Conflict",
            AttributionReason::ExecutableMismatch,
            "",
            "unknown:conflict".to_string(),
        ),
        NotificationAttribution::relay("Relay", "", "relay:relay".to_string()),
    ] {
        assert_eq!(
            attribution.application_action_policy(),
            ApplicationActionPolicy::Deny,
            "status {:?} must not emit application-owned signals",
            attribution.status
        );
    }
}
