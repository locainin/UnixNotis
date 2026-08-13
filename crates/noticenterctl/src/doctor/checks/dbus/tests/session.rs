use crate::doctor::report::DoctorSeverity;

use super::super::session::unavailable_probe;

#[test]
fn unavailable_bus_preserves_the_complete_dependent_check_sequence() {
    let result = unavailable_probe("connection refused".to_string());

    assert!(result.connection.is_none());
    assert_eq!(
        result
            .checks
            .iter()
            .map(|check| check.id.as_str())
            .collect::<Vec<_>>(),
        ["dbus.session", "dbus.dependent-checks"]
    );
    assert_eq!(result.checks[0].severity, DoctorSeverity::Error);
    assert_eq!(result.checks[1].severity, DoctorSeverity::Note);
}
