use super::{AttributionReason, AttributionStatus, IdentityAssurance, NotificationAttribution};
use crate::model::{ApplicationActionPolicy, InlineReplyPolicy, InteractionPolicies};
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
        (AttributionReason::PortalAppIdAssociation, 1),
        (AttributionReason::ExactUserExecutable, 2),
        (AttributionReason::ProtectedPayloadMatch, 3),
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
        (InlineReplyPolicy::Confirm, 1),
        (InlineReplyPolicy::Deny, 2),
    ] {
        let encoded = to_bytes(context, &policy).expect("serialize inline reply policy");
        assert_eq!(InlineReplyPolicy::signature(), u8::signature());
        assert_eq!(encoded.bytes(), &[discriminant]);
    }

    for (assurance, discriminant) in [
        (IdentityAssurance::Authenticated, 0_u8),
        (IdentityAssurance::SystemAssociated, 1),
        (IdentityAssurance::PortalAssociated, 2),
        (IdentityAssurance::UserAssociated, 3),
        (IdentityAssurance::Unresolved, 4),
        (IdentityAssurance::Conflict, 5),
        (IdentityAssurance::Relay, 6),
    ] {
        let encoded = to_bytes(context, &assurance).expect("serialize identity assurance");
        assert_eq!(IdentityAssurance::signature(), u8::signature());
        assert_eq!(encoded.bytes(), &[discriminant]);
    }

    for (policy, discriminant) in [
        (ApplicationActionPolicy::Allow, 0_u8),
        (ApplicationActionPolicy::Confirm, 1),
        (ApplicationActionPolicy::Deny, 2),
    ] {
        let encoded = to_bytes(context, &policy).expect("serialize application action policy");
        assert_eq!(ApplicationActionPolicy::signature(), u8::signature());
        assert_eq!(encoded.bytes(), &[discriminant]);
    }
}

#[test]
fn attribution_wire_enums_reject_unknown_discriminants() {
    let context = Context::new_dbus(LE, 0);
    let unknown = to_bytes(context, &u8::MAX).expect("serialize unknown byte");

    assert!(unknown.deserialize::<AttributionStatus>().is_err());
    assert!(unknown.deserialize::<AttributionReason>().is_err());

    assert!(unknown.deserialize::<InlineReplyPolicy>().is_err());
    assert!(unknown.deserialize::<IdentityAssurance>().is_err());
    assert!(unknown.deserialize::<ApplicationActionPolicy>().is_err());
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
fn authenticated_and_native_policies_keep_action_surfaces_separate() {
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
        verified.action_policy("default"),
        ApplicationActionPolicy::Allow
    );
    assert_eq!(
        verified.action_policy("open"),
        ApplicationActionPolicy::Allow
    );
    assert_eq!(
        verified.action_policy("inline-reply"),
        ApplicationActionPolicy::Deny,
        "even fully verified attributions must reject inline-reply through action dispatch"
    );

    let native = NotificationAttribution::associated(
        "System app",
        "System app",
        "org.example.System",
        "system",
        IdentityAssurance::SystemAssociated,
        InteractionPolicies::NATIVE_COMPATIBILITY,
        AttributionReason::ExactSystemExecutable,
        "",
        "associated:system-app:system".to_string(),
    );
    assert_eq!(
        native.default_activation_policy(),
        ApplicationActionPolicy::Allow,
        "native association should preserve compatible card activation"
    );
    assert_eq!(
        native.action_button_policy(),
        ApplicationActionPolicy::Confirm,
        "native association should require confirmation for richer actions"
    );
    assert_eq!(
        native.action_policy("default"),
        ApplicationActionPolicy::Allow,
        "the protocol default key should use default activation policy"
    );
    assert_eq!(
        native.action_policy("inline-reply"),
        ApplicationActionPolicy::Deny,
        "the inline-reply key must be rejected regardless of button policy"
    );
    assert_eq!(
        native.action_policy("archive"),
        ApplicationActionPolicy::Confirm,
        "non-default keys should use button policy"
    );
    assert_eq!(
        native.interactions.inline_reply,
        InlineReplyPolicy::Deny,
        "same-user native association cannot protect credential-like reply text"
    );
    assert!(native.may_materialize_application_icon());
}

#[test]
fn portal_and_unassociated_policies_never_allow_silent_actions() {
    let portal = NotificationAttribution::associated(
        "Portal app",
        "Portal app",
        "org.example.Portal",
        "portal",
        IdentityAssurance::PortalAssociated,
        InteractionPolicies::CONFIRM_ACTIONS,
        AttributionReason::PortalAppIdAssociation,
        "",
        "associated:portal-app:portal".to_string(),
    );
    assert_eq!(
        portal.default_activation_policy(),
        ApplicationActionPolicy::Confirm,
        "an app id without unforgeable provenance must not activate silently"
    );
    assert!(!portal.may_materialize_application_icon());

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
            attribution.action_policy("default"),
            ApplicationActionPolicy::Deny,
            "status {:?} must not emit application-owned signals",
            attribution.status
        );
    }
}

#[test]
fn host_visuals_do_not_require_action_authority() {
    let mut authenticated = NotificationAttribution::verified(
        "Example",
        "Example",
        "org.example.App",
        "example",
        AttributionReason::ExactSystemExecutable,
        "",
        "verified:example".to_string(),
    );
    authenticated.interactions = InteractionPolicies::DENY;

    assert!(authenticated.may_materialize_application_icon());
}

#[test]
fn verification_status_is_not_inferred_from_display_fields() {
    let verified = NotificationAttribution::verified(
        "Example",
        "Example",
        "org.example.App",
        "example",
        AttributionReason::ExactSystemExecutable,
        "",
        "verified:example".to_string(),
    );
    let unresolved = NotificationAttribution::unresolved(
        "Example",
        AttributionReason::MissingSenderEvidence,
        "",
        "unknown:example".to_string(),
    );

    assert!(verified.is_verified());
    assert!(!unresolved.is_verified());
}
