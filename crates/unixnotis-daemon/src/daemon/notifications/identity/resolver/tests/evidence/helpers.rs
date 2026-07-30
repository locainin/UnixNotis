//! Helper-process and process-lineage association cases

use super::super::super::evidence::{lineage_association, sender_claim_relation};
use super::super::*;

#[test]
fn helper_process_lineage_is_recognized_without_becoming_suspicious() {
    let (app_path, app_identity) = installed_system_executable();
    let index = DesktopIdentityIndex::from_records(
        vec![
            system_record("org.example.True", "Example App", &app_path, app_identity)
                .with_launch_literals(&["--application-mode"]),
        ],
        Vec::new(),
    );
    let helper_identity = identity(88, 880, 0);
    let mut helper = sender("/usr/libexec/example-helper", helper_identity);
    helper.ancestors.push(ProcessLineageEvidence {
        pid: 8_080,
        start_time: 7_070,
        uid: 0,
        executable: app_path,
        executable_identity: app_identity,
    });

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Example App",
            desktop_entry: Some("org.example.True"),
        },
        &helper,
        &index,
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Recognized);
    assert_eq!(resolution.attribution.display_name, "Example App");
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert_ne!(resolution.attribution.status, AttributionStatus::Conflict);
    assert!(resolution
        .attribution
        .diagnostic_detail
        .contains("Same-user ancestor"));
}

#[test]
fn stale_ancestor_identity_does_not_create_a_lineage_association() {
    let (app_path, live_identity) = installed_system_executable();
    let stale_identity = FileIdentity {
        inode: live_identity.inode.saturating_add(1),
        ..live_identity
    };
    let record = system_record("org.example.True", "Example App", &app_path, stale_identity)
        .with_launch_literals(&["--application-mode"]);
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());
    let mut helper = sender("/usr/libexec/example-helper", identity(204, 2_040, 0));
    helper.ancestors.push(ProcessLineageEvidence {
        pid: 8_081,
        start_time: 7_071,
        uid: 0,
        executable: app_path,
        executable_identity: stale_identity,
    });

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Example App",
            desktop_entry: Some("org.example.True"),
        },
        &helper,
        &index,
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Unresolved);
    assert_eq!(resolution.attribution.display_name, "Unknown application");
    assert!(!resolution
        .attribution
        .diagnostic_detail
        .contains("Same-user ancestor"));
}

#[test]
fn lineage_rejects_a_candidate_with_a_different_indexed_executable() {
    let (app_path, live_identity) = installed_system_executable();
    let indexed = system_record("org.example.True", "Example App", &app_path, live_identity)
        .with_launch_literals(&["--application-mode"]);
    let index = DesktopIdentityIndex::from_records(vec![indexed.clone()], Vec::new());
    let mut mismatched = indexed;
    mismatched.runtime_executable_identity = Some(FileIdentity {
        inode: live_identity.inode.saturating_add(1),
        ..live_identity
    });
    let result = CandidateVerification {
        record: &mismatched,
        verification: LaunchVerification::DefinitiveMismatch(LaunchFailure::ExecutableMismatch),
    };
    let mut helper = sender("/usr/libexec/example-helper", identity(206, 2_060, 0));
    helper.ancestors.push(ProcessLineageEvidence {
        pid: 8_082,
        start_time: 7_072,
        uid: 0,
        executable: app_path,
        executable_identity: live_identity,
    });

    assert!(lineage_association(&helper, &index, &[&result]).is_none());
}

#[test]
fn direct_protected_payload_without_command_line_is_not_lineage_evidence() {
    let (app_path, app_identity) = installed_system_executable();
    let (payload_path, payload_identity) = installed_system_executable();
    let indexed = system_record("org.example.True", "Example App", &app_path, app_identity)
        .with_launch_literals(&[&payload_path])
        .with_protected_launch_file(&payload_path, payload_identity);
    let index = DesktopIdentityIndex::from_records(vec![indexed], Vec::new());
    let record = index
        .records_for_id("org.example.True")
        .into_iter()
        .next()
        .expect("protected-payload record should be indexed");
    let result = CandidateVerification {
        record,
        verification: LaunchVerification::InsufficientEvidence(LaunchFailure::MissingCommandLine),
    };
    let mut helper = sender("/usr/libexec/example-helper", identity(207, 2_070, 0));
    helper.ancestors.push(ProcessLineageEvidence {
        pid: 8_083,
        start_time: 7_073,
        uid: 0,
        executable: app_path,
        executable_identity: app_identity,
    });

    assert!(
        lineage_association(&helper, &index, &[&result]).is_none(),
        "missing command-line evidence is accepted only for a validated package launcher"
    );
}

#[test]
fn unknown_executable_cannot_borrow_installed_app_identity() {
    let (app_path, app_identity) = installed_system_executable();
    let index = DesktopIdentityIndex::from_records(
        vec![system_record(
            "org.example.True",
            "Example App",
            &app_path,
            app_identity,
        )],
        Vec::new(),
    );
    let helper = sender("/tmp/random-script", identity(89, 890, 1_000));

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Example App",
            desktop_entry: Some("org.example.True"),
        },
        &helper,
        &index,
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Unresolved);
    assert_eq!(resolution.attribution.display_name, "Unknown application");
    assert_eq!(resolution.attribution.claimed_name, "Example App");
    assert!(resolution.attribution.desktop_id.is_empty());
    assert_eq!(
        resolution.attribution.badge_icon,
        "application-x-executable-symbolic"
    );
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn verified_and_unresolved_senders_never_share_an_application_group() {
    let (app_path, app_identity) = installed_system_executable();
    let index = DesktopIdentityIndex::from_records(
        vec![system_record(
            "org.example.True",
            "Example App",
            &app_path,
            app_identity,
        )],
        Vec::new(),
    );
    let verified = resolve_with_evidence(
        AppClaim {
            reported_name: "Example App",
            desktop_entry: Some("org.example.True"),
        },
        &sender(&app_path, app_identity),
        &index,
    );
    let unresolved = resolve_with_evidence(
        AppClaim {
            reported_name: "Example App",
            desktop_entry: Some("org.example.True"),
        },
        &sender("/opt/example/helper", identity(90, 900, 1_000)),
        &index,
    );

    assert_eq!(verified.attribution.status, AttributionStatus::Recognized);
    assert_eq!(
        verified.attribution.assurance,
        unixnotis_core::IdentityAssurance::SystemAssociated
    );
    assert_eq!(unresolved.attribution.status, AttributionStatus::Unresolved);
    assert_ne!(
        verified.attribution.group_key, unresolved.attribution.group_key,
        "different trust domains must remain separate even for one canonical application"
    );
}

#[test]
fn same_package_helper_is_recognized() {
    let (app_path, app_identity) = installed_system_executable();
    let index = DesktopIdentityIndex::from_records(
        vec![system_record(
            "org.example.True",
            "Example App",
            &app_path,
            app_identity,
        )],
        Vec::new(),
    );
    let mut helper = sender("/usr/lib/example/helper", identity(91, 910, 0));
    helper.install_provenance = package("org.example.True");

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Example App",
            desktop_entry: Some("org.example.True"),
        },
        &helper,
        &index,
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Recognized);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    let claimed_record = index
        .records_for_id("org.example.True")
        .into_iter()
        .next()
        .expect("claimed record should be indexed");
    assert_eq!(
        sender_claim_relation(&helper, &index, claimed_record),
        SenderClaimRelation::SamePackageHelper
    );
    assert!(resolution
        .attribution
        .diagnostic_detail
        .contains("same installed application package"));
}

#[test]
fn different_package_cannot_borrow_installed_app_identity() {
    let (app_path, app_identity) = installed_system_executable();
    let index = DesktopIdentityIndex::from_records(
        vec![system_record(
            "org.example.True",
            "Example App",
            &app_path,
            app_identity,
        )],
        Vec::new(),
    );
    let mut different_package = sender("/usr/libexec/example-helper", identity(92, 920, 0));
    different_package.install_provenance = package("org.example.Integration");

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Example App",
            desktop_entry: Some("org.example.True"),
        },
        &different_package,
        &index,
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Unresolved);
    assert_eq!(resolution.attribution.display_name, "Unknown application");
    assert_eq!(resolution.attribution.claimed_name, "Example App");
    assert!(resolution.attribution.desktop_id.is_empty());
    assert_eq!(
        resolution.attribution.badge_icon,
        "application-x-executable-symbolic"
    );
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert_eq!(
        resolution.diagnostics.verification,
        LaunchVerificationView::InsufficientEvidence,
        "the launch mismatch remains diagnostic evidence without proving impersonation"
    );
    let claimed_record = index
        .records_for_id("org.example.True")
        .into_iter()
        .next()
        .expect("claimed record should be indexed");
    assert_eq!(
        sender_claim_relation(&different_package, &index, claimed_record),
        SenderClaimRelation::DifferentInstalledPackage
    );
}

#[test]
fn verified_different_application_is_conflict() {
    let (app_path, app_identity) = installed_system_executable();
    let other_identity = identity(93, 930, 0);
    let index = DesktopIdentityIndex::from_records(
        vec![
            system_record("org.example.True", "Example App", &app_path, app_identity),
            system_record(
                "org.example.Other",
                "Other App",
                "/usr/bin/other-app",
                other_identity,
            ),
        ],
        Vec::new(),
    );

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Example App",
            desktop_entry: Some("org.example.True"),
        },
        &sender("/usr/bin/other-app", other_identity),
        &index,
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Conflict);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert_eq!(
        resolution.diagnostics.verification,
        LaunchVerificationView::DefinitiveMismatch
    );
}

#[test]
fn user_record_owning_sender_executable_cannot_prove_a_conflict() {
    let claimed_identity = identity(104, 1_040, 0);
    let user_identity = identity(105, 1_050, 1_000);
    let claimed = system_record(
        "org.example.Claimed",
        "Claimed App",
        "/usr/bin/claimed",
        claimed_identity,
    );
    let user = DesktopRecord::fixture(
        "org.example.Local",
        "Local App",
        "/home/user/bin/local",
        user_identity,
        false,
    );
    let index = DesktopIdentityIndex::from_records(vec![claimed, user], Vec::new());
    let claimed_record = index
        .records_for_id("org.example.Claimed")
        .into_iter()
        .next()
        .expect("claimed record should be indexed");

    assert_eq!(
        sender_claim_relation(
            &sender("/home/user/bin/local", user_identity),
            &index,
            claimed_record,
        ),
        SenderClaimRelation::UnknownExecutable,
        "a user desktop record is not immutable contradictory ownership"
    );
}
