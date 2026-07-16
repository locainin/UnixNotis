use crate::service_manager::ReadinessIssue;

#[test]
fn readiness_issue_preserves_severity_and_message() {
    let warning = ReadinessIssue::warning("optional manager hint");
    let error = ReadinessIssue::error("required manager setup");

    assert!(!warning.is_error());
    assert_eq!(warning.message(), "optional manager hint");
    assert!(error.is_error());
    assert_eq!(error.message(), "required manager setup");
}
