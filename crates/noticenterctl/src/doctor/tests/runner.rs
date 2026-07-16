use crate::doctor::model::{
    DoctorCheck, DoctorLogResult, DoctorLogSource, DoctorReport, DoctorSeverity,
};

#[test]
fn ordered_check_aggregation_keeps_insertion_order_and_error_semantics() {
    let checks = vec![
        DoctorCheck::new("environment", "Environment", DoctorSeverity::Pass, "ready"),
        DoctorCheck::new("logs", "Logs", DoctorSeverity::Note, "unavailable"),
        DoctorCheck::new("dbus", "D-Bus", DoctorSeverity::Error, "missing"),
    ];
    let report = DoctorReport::new(
        checks,
        DoctorLogResult::Unavailable {
            source: DoctorLogSource::Unknown,
            reason: "unknown".to_string(),
            hint: None,
        },
    );

    assert_eq!(report.checks[0].id, "environment");
    assert_eq!(report.checks[1].id, "logs");
    assert_eq!(report.checks[2].id, "dbus");
    assert!(report.has_errors());
}
