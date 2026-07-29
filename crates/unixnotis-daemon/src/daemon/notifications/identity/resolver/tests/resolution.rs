//! Attribution construction and grouping tests

use super::super::resolution::{
    resolution_for_record, sender_claim_group_key, unknown_reply_denied,
};
use super::*;
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
fn verified_package_launcher_target_receives_application_authority() {
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

    assert_eq!(resolution.attribution.status, AttributionStatus::Verified);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Allow);
    assert_eq!(
        resolution.attribution.application_action_policy(),
        ApplicationActionPolicy::Allow
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
