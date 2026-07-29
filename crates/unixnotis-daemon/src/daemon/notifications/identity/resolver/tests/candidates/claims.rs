//! Application-name and desktop-hint association cases

use super::super::*;

#[test]
fn mismatched_desktop_hint_does_not_become_claim_evidence() {
    let protected_identity = identity(100, 1_000, 0);
    let record = system_record(
        "org.example.Protected",
        "Protected App",
        "/usr/bin/protected",
        protected_identity,
    );
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());
    let hint_records = index.records_for_id("org.example.Protected");
    let results = hint_records
        .iter()
        .map(|record| CandidateVerification {
            record,
            verification: LaunchVerification::DefinitiveMismatch(LaunchFailure::ExecutableMismatch),
        })
        .collect::<Vec<_>>();
    let mut different = sender("/usr/bin/different", identity(101, 1_010, 0));
    different.install_provenance = package("different-app");

    let resolution = resolve_unverified_candidates(
        AppClaim {
            reported_name: "Unrelated label",
            desktop_entry: Some("org.example.Protected"),
        },
        &different,
        &index,
        &hint_records,
        &results,
    );

    assert_eq!(
        resolution.attribution.status,
        AttributionStatus::Unresolved,
        "a caller-controlled desktop hint cannot make a different label contradictory"
    );
}

#[test]
fn canonical_conflict_candidate_supplies_the_stable_failure_reason() {
    let executable = identity(102, 1_020, 0);
    let mut canonical = system_record(
        "org.example.Canonical",
        "Example App",
        "/usr/bin/example",
        executable,
    );
    let mut alias = system_record(
        "org.example.Canonical.NewWindow",
        "Example App",
        "/usr/bin/example",
        executable,
    );
    for record in [&mut canonical, &mut alias] {
        record.desktop_provenance = package("example-app");
        record.declared_executable_provenance = package("example-app");
        record.runtime_executable_provenance = package("example-app");
    }
    let different_identity = identity(103, 1_030, 0);
    let different_record = system_record(
        "org.example.Different",
        "Different App",
        "/usr/bin/different",
        different_identity,
    );
    let index =
        DesktopIdentityIndex::from_records(vec![alias, canonical, different_record], Vec::new());
    let records = index.records_for_executable(executable);
    let results = records
        .iter()
        .map(|record| CandidateVerification {
            record,
            verification: if record.id == "org.example.Canonical" {
                LaunchVerification::DefinitiveMismatch(LaunchFailure::ExecutableMismatch)
            } else {
                LaunchVerification::DefinitiveMismatch(LaunchFailure::ProtectedPayloadMismatch)
            },
        })
        .collect::<Vec<_>>();
    let different = sender("/usr/bin/different", different_identity);

    let resolution = resolve_unverified_candidates(
        AppClaim {
            reported_name: "Example App",
            desktop_entry: None,
        },
        &different,
        &index,
        &[],
        &results,
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Conflict);
    assert_eq!(
        resolution.attribution.reason,
        unixnotis_core::AttributionReason::ExecutableMismatch
    );
}
