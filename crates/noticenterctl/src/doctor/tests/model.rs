use super::*;

#[test]
fn severity_labels_remain_stable_for_human_reports() {
    assert_eq!(DoctorSeverity::Pass.label(), "pass");
    assert_eq!(DoctorSeverity::Note.label(), "note");
    assert_eq!(DoctorSeverity::Warning.label(), "warning");
    assert_eq!(DoctorSeverity::Error.label(), "error");
}

#[test]
fn errors_alone_make_the_doctor_report_unsuccessful() {
    let note = DoctorCheck::new("logs", "Logs", DoctorSeverity::Note, "unavailable");
    let warning = DoctorCheck::new("css", "CSS", DoctorSeverity::Warning, "incomplete");
    let logs = DoctorLogResult::Unavailable {
        source: DoctorLogSource::Dinit,
        reason: "not configured".to_string(),
        hint: None,
    };
    assert!(!DoctorReport::new(vec![note, warning], logs.clone()).has_errors());

    let error = DoctorCheck::new("config", "Config", DoctorSeverity::Error, "rejected");
    assert!(DoctorReport::new(vec![error], logs).has_errors());
}

#[test]
fn json_model_keeps_schema_and_tagged_log_status_stable() {
    let report = DoctorReport::new(
        Vec::new(),
        DoctorLogResult::Collected {
            source: DoctorLogSource::SystemdJournal,
            lines: vec!["ready".to_string()],
        },
    );
    let value = serde_json::to_value(report).expect("serialize doctor report");

    assert_eq!(value["schema_version"], DOCTOR_SCHEMA_VERSION);
    assert_eq!(value["logs"]["status"], "collected");
    assert_eq!(value["logs"]["source"], "systemd_journal");
}
