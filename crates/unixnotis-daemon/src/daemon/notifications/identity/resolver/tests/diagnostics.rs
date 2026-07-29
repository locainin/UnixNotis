//! Structured resolver diagnostic regressions

use super::super::diagnostics::{launch_failure_label, with_diagnostics};
use super::super::resolution::recognized_resolution;
use super::*;

#[test]
fn empty_contract_diagnostic_explains_why_command_line_evidence_is_required() {
    assert_eq!(
        launch_failure_label(LaunchFailure::EmptyContractNeedsCommandLine),
        "empty launch contract requires structured command-line evidence"
    );
}

#[test]
fn nonconflicting_mismatch_is_reported_as_insufficient_evidence() {
    let record = system_record(
        "org.example.App",
        "Example App",
        "/usr/bin/example-app",
        identity(202, 2_020, 0),
    );
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());
    let record = index
        .records_for_id("org.example.App")
        .into_iter()
        .next()
        .expect("fixture record should be indexed");
    let metadata = sender("/usr/libexec/example-helper", identity(203, 2_030, 0));
    let claim = AppClaim {
        reported_name: "Example App",
        desktop_entry: Some("org.example.App"),
    };
    let resolution = recognized_resolution(
        claim,
        &metadata,
        record,
        &index,
        LaunchFailure::ExecutableMismatch,
        "helper could not be strongly bound",
    );

    let resolution = with_diagnostics(
        resolution,
        claim,
        &metadata,
        Some(record),
        LaunchVerification::DefinitiveMismatch(LaunchFailure::ExecutableMismatch),
    );

    assert_eq!(resolution.attribution.status, AttributionStatus::Recognized);
    assert_eq!(
        resolution.diagnostics.verification,
        LaunchVerificationView::InsufficientEvidence
    );
}
