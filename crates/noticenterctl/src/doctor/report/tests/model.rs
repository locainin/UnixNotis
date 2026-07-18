use super::super::model::*;

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
    assert!(!DoctorReport::new(vec![note, warning], Vec::new(), logs.clone()).has_errors());

    let error = DoctorCheck::new("config", "Config", DoctorSeverity::Error, "rejected");
    assert!(DoctorReport::new(vec![error], Vec::new(), logs).has_errors());
}

#[test]
fn json_model_keeps_schema_and_tagged_log_status_stable() {
    let report = DoctorReport::new(
        Vec::new(),
        Vec::new(),
        DoctorLogResult::Collected {
            source: DoctorLogSource::SystemdJournal,
            lines: vec!["ready".to_string()],
            truncated: false,
            line_limit: 30,
            byte_limit: 32 * 1024,
        },
    );
    let value = serde_json::to_value(report).expect("serialize doctor report");

    assert_eq!(value["schema_version"], DOCTOR_SCHEMA_VERSION);
    assert_eq!(value["logs"]["status"], "collected");
    assert_eq!(value["logs"]["source"], "systemd_journal");
    assert_eq!(value["config_diagnostics"], serde_json::json!([]));
}

#[test]
fn json_preserves_config_diagnostics_as_typed_stable_records() {
    let diagnostic = unixnotis_core::ConfigDiagnostic {
        code: "config.unknown-key",
        kind: unixnotis_core::ConfigDiagnosticKind::Warning,
        path: Some("panel.unknown".to_string()),
        message: "Unknown configuration key was ignored".to_string(),
        original: None,
        effective: None,
    };
    let report = DoctorReport::new(
        Vec::new(),
        vec![diagnostic],
        DoctorLogResult::Unavailable {
            source: DoctorLogSource::Unknown,
            reason: "not configured".to_string(),
            hint: None,
        },
    );

    let value = serde_json::to_value(report).expect("serialize typed config diagnostic");

    assert_eq!(value["config_diagnostics"][0]["code"], "config.unknown-key");
    assert_eq!(value["config_diagnostics"][0]["path"], "panel.unknown");
    assert!(value["checks"].as_array().is_some_and(Vec::is_empty));
}
