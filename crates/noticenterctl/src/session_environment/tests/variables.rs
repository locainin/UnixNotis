use std::ffi::OsString;

use super::super::variables::{missing_session_variables, validate_session_environment};

#[test]
fn session_environment_reports_empty_required_values_as_missing() {
    let missing = missing_session_variables(|name| match name {
        "WAYLAND_DISPLAY" => Some(OsString::from("wayland-1")),
        "XDG_RUNTIME_DIR" => Some(OsString::new()),
        _ => None,
    });

    assert_eq!(missing, vec!["XDG_RUNTIME_DIR"]);
}

#[test]
fn session_environment_reports_every_absent_required_value() {
    let missing = missing_session_variables(|_| None);

    assert_eq!(missing, vec!["WAYLAND_DISPLAY", "XDG_RUNTIME_DIR"]);
}

#[test]
fn complete_session_environment_passes_validation() {
    let missing = missing_session_variables(|_| Some(OsString::from("present")));

    assert!(missing.is_empty());
    validate_session_environment(|_| Some(OsString::from("present")))
        .expect("complete session environment");
}

#[test]
fn missing_session_environment_returns_an_actionable_error() {
    let error = validate_session_environment(|_| None)
        .expect_err("missing session values must be rejected");

    assert!(error.to_string().contains("WAYLAND_DISPLAY"));
    assert!(error.to_string().contains("XDG_RUNTIME_DIR"));
}
