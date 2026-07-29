use super::super::super::evidence::sender_claim_relation;
use super::super::*;

#[test]
fn helper_process_lineage_is_recognized_without_becoming_suspicious() {
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
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Recognized);
    assert_eq!(resolution.attribution.display_name, "Example App");
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert_ne!(resolution.attribution.status, AttributionStatus::Conflict);
}

#[test]
fn helper_without_lineage_is_recognized_when_no_contradictory_owner_is_known() {
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
    let helper = sender("/opt/example/helper", identity(89, 890, 1_000));

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Example App",
            desktop_entry: Some("org.example.True"),
        },
        &helper,
        &index,
        &HashSet::new(),
    );

    assert_eq!(
        resolution.attribution.status,
        AttributionStatus::Recognized,
        "missing lineage cannot prove that a helper belongs to another application"
    );
    assert_eq!(resolution.attribution.display_name, "Example App");
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn verified_and_recognized_senders_never_share_an_application_group() {
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
        &HashSet::new(),
    );
    let recognized = resolve_with_evidence(
        AppClaim {
            reported_name: "Example App",
            desktop_entry: Some("org.example.True"),
        },
        &sender("/opt/example/helper", identity(90, 900, 1_000)),
        &index,
        &HashSet::new(),
    );

    assert_eq!(verified.attribution.status, AttributionStatus::Verified);
    assert_eq!(recognized.attribution.status, AttributionStatus::Recognized);
    assert_ne!(
        verified.attribution.group_key, recognized.attribution.group_key,
        "different trust domains must remain separate even for one canonical application"
    );
}

#[test]
fn package_owned_helper_for_the_claimed_application_is_recognized() {
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
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Recognized);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
}

#[test]
fn different_verified_package_is_concrete_conflict_evidence() {
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
    let mut different = sender("/usr/bin/different-app", identity(92, 920, 0));
    different.install_provenance = package("org.example.Different");

    let resolution = resolve_with_evidence(
        AppClaim {
            reported_name: "Example App",
            desktop_entry: Some("org.example.True"),
        },
        &different,
        &index,
        &HashSet::new(),
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Conflict);
    assert_eq!(resolution.inline_reply_policy, InlineReplyPolicy::Deny);
    assert_eq!(
        resolution.diagnostics.verification,
        LaunchVerificationView::DefinitiveMismatch,
        "only the concrete different-package relation should remain a definitive mismatch"
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
