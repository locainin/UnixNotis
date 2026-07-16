use super::super::render::*;
use crate::doctor::report::{DoctorCheck, DoctorLogResult, DoctorLogSource, DoctorSeverity};

use super::super::model::DoctorReport;

#[test]
fn human_output_renders_details_hints_and_unavailable_logs() {
    let report = DoctorReport::new(
        vec![
            DoctorCheck::new("service", "Service", DoctorSeverity::Pass, "active")
                .details("Manager: dinit")
                .hint("none"),
        ],
        Vec::new(),
        DoctorLogResult::Unavailable {
            source: DoctorLogSource::Dinit,
            reason: "The installed dinit service does not configure a persistent log buffer"
                .to_string(),
            hint: Some("run unixnotis-daemon in a terminal".to_string()),
        },
    );
    let rendered = render_human(&report);

    assert!(rendered.contains("SERVICE"));
    assert!(rendered.contains("Manager: dinit"));
    assert!(rendered.contains("Persistent logs: unavailable"));
}

#[test]
fn json_output_is_valid_and_versioned() {
    let report = DoctorReport::new(
        Vec::new(),
        Vec::new(),
        DoctorLogResult::Collected {
            source: DoctorLogSource::SystemdJournal,
            lines: Vec::new(),
            truncated: false,
            line_limit: 30,
            byte_limit: 32 * 1024,
        },
    );
    let rendered = render_json(&report).expect("render json");
    let value: serde_json::Value = serde_json::from_str(&rendered).expect("valid json");

    assert_eq!(value["schema_version"], 1);
}
