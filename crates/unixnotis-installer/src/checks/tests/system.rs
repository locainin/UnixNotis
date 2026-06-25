use std::path::PathBuf;

use crate::service_manager::{ReadinessIssue, ServiceManager};

use super::system::{readiness_error_detail, readiness_messages, readiness_warning_detail};

#[test]
fn readiness_error_detail_collects_only_blocking_issues() {
    let issues = [
        ReadinessIssue::warning("boot setup incomplete"),
        ReadinessIssue::error("s6-db-reload not found"),
        ReadinessIssue::error("s6 live directory missing"),
    ];

    let detail = readiness_error_detail(&issues).expect("blocking detail");

    assert!(detail.contains("s6-db-reload not found"));
    assert!(detail.contains("s6 live directory missing"));
    assert!(!detail.contains("boot setup incomplete"));
}

#[test]
fn readiness_warning_detail_keeps_backend_label() {
    let manager = ServiceManager::dinit_user(PathBuf::from("/tmp/dinit.d"));
    let issues = [ReadinessIssue::warning("boot setup incomplete")];

    let detail = readiness_warning_detail(&manager, &issues).expect("warning detail");

    assert_eq!(
        detail,
        "dinit --user ready with warnings: boot setup incomplete"
    );
}

#[test]
fn readiness_messages_split_warnings_and_errors() {
    let issues = [
        ReadinessIssue::warning("warning one"),
        ReadinessIssue::error("error one"),
        ReadinessIssue::warning("warning two"),
    ];

    assert_eq!(
        readiness_messages(&issues, false),
        ["warning one".to_string(), "warning two".to_string()]
    );
    assert_eq!(readiness_messages(&issues, true), ["error one".to_string()]);
}
