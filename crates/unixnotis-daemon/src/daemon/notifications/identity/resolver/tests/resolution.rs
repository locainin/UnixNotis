//! Attribution construction and grouping tests

use super::super::resolution::{
    owner_bound_default_interactions, recognized_resolution, resolution_for_record,
    sender_claim_group_key, unknown_reply_denied,
};
use super::*;
use crate::daemon::notifications::identity::sender::SenderMetadataStatus;
use unixnotis_core::ApplicationActionPolicy;

#[test]
fn sender_claim_group_key_is_nonempty_and_bound_to_sender_identity() {
    let metadata = sender("/usr/bin/example", identity(106, 1_060, 0));

    let unresolved =
        sender_claim_group_key(AttributionStatus::Unresolved, "Example App", &metadata);
    let conflict = sender_claim_group_key(AttributionStatus::Conflict, "Example App", &metadata);

    assert_eq!(unresolved, "unresolved:106:1060:exampleapp");
    assert_eq!(conflict, "conflict:106:1060:exampleapp");
    assert_ne!(unresolved, conflict);
}

#[test]
fn verified_record_with_a_contradictory_name_becomes_conflict() {
    let record = system_record(
        "org.example.App",
        "Example App",
        "/usr/bin/example-app",
        identity(205, 2_050, 0),
    );
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());
    let record = index
        .records_for_id("org.example.App")
        .into_iter()
        .next()
        .expect("fixture record should be indexed");
    let resolution = resolution_for_record(
        VerifiedDesktopRecord(record, VerifiedLaunch::DedicatedExecutable),
        "Different App",
        &sender("/usr/bin/example-app", identity(205, 2_050, 0)),
        &index,
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Conflict);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn package_launcher_target_preserves_only_compatible_default_activation() {
    let record = system_record(
        "org.example.App",
        "Example App",
        "/usr/lib/example-app/runtime",
        identity(207, 2_070, 0),
    );
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());
    let record = index
        .records_for_id("org.example.App")
        .into_iter()
        .next()
        .expect("fixture record should be indexed");

    let resolution = resolution_for_record(
        VerifiedDesktopRecord(record, VerifiedLaunch::PackageLauncherTarget),
        "Example App",
        &sender("/usr/lib/example-app/runtime", identity(207, 2_070, 0)),
        &index,
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Recognized);
    assert_eq!(
        resolution.attribution.assurance,
        unixnotis_core::IdentityAssurance::SystemAssociated
    );
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert_eq!(
        resolution.attribution.default_activation_policy(),
        ApplicationActionPolicy::Allow
    );
    assert_eq!(
        resolution.attribution.action_button_policy(),
        ApplicationActionPolicy::Confirm
    );
}

#[test]
fn missing_sender_reply_resolution_is_unresolved_and_noninteractive() {
    let metadata = SenderMetadata::default();
    let resolution = unknown_reply_denied(
        AppClaim {
            reported_name: "Example App",
            desktop_entry: None,
        },
        &metadata,
        "sender metadata unavailable",
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Unresolved);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert!(resolution
        .attribution
        .diagnostic_detail
        .contains("sender metadata unavailable"));
}

#[test]
fn sender_credential_timeout_is_preserved_in_diagnostics() {
    let metadata = SenderMetadata {
        status: SenderMetadataStatus::CredentialLookupTimedOut,
        ..SenderMetadata::default()
    };
    let resolution = unknown_reply_denied(
        AppClaim {
            reported_name: "Example Chat",
            desktop_entry: None,
        },
        &metadata,
        "sender metadata unavailable",
    );

    assert!(resolution
        .attribution
        .diagnostic_detail
        .contains("credential lookup timed out"));
}

#[test]
fn stable_callback_owner_gets_only_default_activation_authority() {
    let mut metadata = sender("/usr/bin/example", identity(106, 1_060, 0));
    metadata.sender_pid = Some(42);
    metadata.sender_start_time = Some(4_200);
    metadata.sender_uid = Some(1_000);

    assert_eq!(
        owner_bound_default_interactions(&metadata),
        InteractionPolicies::OWNER_BOUND_DEFAULT
    );
}

#[test]
fn incomplete_callback_owner_gets_no_interaction_authority() {
    let metadata = sender("/usr/bin/example", identity(106, 1_060, 0));

    assert_eq!(
        owner_bound_default_interactions(&metadata),
        InteractionPolicies::DENY
    );
}

#[test]
fn credential_timeout_never_gets_owner_bound_default_authority() {
    let metadata = SenderMetadata {
        sender_name: Some(":1.43".to_string()),
        sender_pid: Some(43),
        sender_start_time: Some(4_300),
        sender_uid: Some(1_000),
        status: SenderMetadataStatus::CredentialLookupTimedOut,
        ..SenderMetadata::default()
    };

    assert_eq!(
        owner_bound_default_interactions(&metadata),
        InteractionPolicies::DENY
    );
}

#[test]
fn recognized_candidate_with_a_stable_owner_exposes_only_default_activation() {
    let record = system_record(
        "org.example.Application",
        "Example Application",
        "/usr/bin/example",
        identity(107, 1_070, 0),
    );
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());
    let record = index
        .records_for_id("org.example.Application")
        .into_iter()
        .next()
        .expect("fixture record should be indexed");
    let mut metadata = sender("/usr/bin/example", identity(107, 1_070, 0));
    metadata.sender_pid = Some(43);
    metadata.sender_start_time = Some(4_300);
    metadata.sender_uid = Some(1_000);

    let resolution = recognized_resolution(
        AppClaim {
            reported_name: "Example Application",
            desktop_entry: None,
        },
        &metadata,
        record,
        &index,
        LaunchFailure::ExecutableMismatch,
        "generic stable-owner fixture",
    );

    assert_eq!(
        resolution.attribution.interactions,
        InteractionPolicies::OWNER_BOUND_DEFAULT
    );
}
