use crate::doctor::report::DoctorSeverity;

use super::super::classify::control_state_failure_check;

#[test]
fn access_denied_state_failure_explains_installed_client_requirements() {
    let error = zbus::Error::FDO(Box::new(zbus::fdo::Error::AccessDenied(
        "caller is not authorized for control operation".to_string(),
    )));

    let check = control_state_failure_check(&error);

    assert_eq!(check.id, "dbus.control-state");
    assert_eq!(check.severity, DoctorSeverity::Error);
    assert_eq!(check.summary, "UnixNotis control access denied");
    assert_eq!(
        check.details.as_deref(),
        Some("The running daemon rejected this client")
    );
    assert!(check
        .hint
        .as_deref()
        .is_some_and(|hint| hint.contains("installed noticenterctl")));
    assert!(!check
        .details
        .as_deref()
        .is_some_and(|details| details.contains("caller is not authorized")));
}

#[test]
fn non_authorization_state_failure_preserves_the_original_error() {
    let error = zbus::Error::Failure("state unavailable".to_string());

    let check = control_state_failure_check(&error);

    assert_eq!(check.summary, "GetState failed");
    assert_eq!(check.details.as_deref(), Some("state unavailable"));
    assert!(check.hint.is_none());
}
