//! Deterministic doctor report rendering

use anyhow::Result;

use super::model::{DoctorLogResult, DoctorReport};
use super::text::safe_doctor_text;

pub(super) fn render_json(report: &DoctorReport) -> Result<String> {
    // Pretty JSON remains readable as an issue attachment while preserving the schema
    Ok(serde_json::to_string_pretty(report)?)
}

pub(super) fn render_human(report: &DoctorReport) -> String {
    // Human output is a terminal boundary for paths, errors, config keys, and logs
    // Keep every free-form model value bounded and on one physical line
    // The heading includes both software and report schema versions
    let mut lines = vec![format!(
        "UnixNotis doctor {} (schema {})",
        safe_doctor_text(&report.unixnotis_version),
        report.schema_version
    )];

    // Input order is retained so related checks stay grouped predictably
    for check in &report.checks {
        lines.push(String::new());
        lines.push(safe_doctor_text(&check.label).to_uppercase());
        lines.push(format!(
            "[{}] {}",
            check.severity.label(),
            safe_doctor_text(&check.summary)
        ));
        // Optional context stays on plain lines for easy terminal copying
        if let Some(details) = &check.details {
            lines.push(safe_doctor_text(details));
        }
        if let Some(hint) = &check.hint {
            lines.push(format!("Hint: {}", safe_doctor_text(hint)));
        }
    }

    if !report.config_diagnostics.is_empty() {
        lines.push(String::new());
        lines.push("CONFIGURATION DIAGNOSTICS".to_string());
        for diagnostic in &report.config_diagnostics {
            lines.push(format!(
                "[{:?}] {}",
                diagnostic.kind,
                safe_doctor_text(&diagnostic.message)
            ));
            lines.push(format!("Code: {}", diagnostic.code));
            if let Some(path) = &diagnostic.path {
                lines.push(format!("Key: {}", safe_doctor_text(path)));
            }
            if let Some(original) = &diagnostic.original {
                lines.push(format!("Original: {}", safe_doctor_text(original)));
            }
            if let Some(effective) = &diagnostic.effective {
                lines.push(format!("Effective: {}", safe_doctor_text(effective)));
            }
        }
    }

    lines.push(String::new());
    lines.push("LOGS".to_string());
    // Tagged log outcomes map to one compact human section
    match &report.logs {
        DoctorLogResult::Collected {
            source,
            lines: logs,
            truncated,
            line_limit,
            byte_limit,
        } => {
            // Indentation distinguishes collected log lines from report fields
            lines.push(format!("Source: {source:?}"));
            lines.push(format!("Limits: {line_limit} lines, {byte_limit} bytes"));
            lines.push(format!("Truncated: {truncated}"));
            lines.extend(
                logs.iter()
                    .map(|line| format!("  {}", safe_doctor_text(line))),
            );
        }
        DoctorLogResult::Unavailable { reason, hint, .. } => {
            // Unavailable sources explain the limitation without pretending collection failed
            lines.push("Persistent logs: unavailable".to_string());
            lines.push(safe_doctor_text(reason));
            if let Some(hint) = hint {
                lines.push(format!("Hint: {}", safe_doctor_text(hint)));
            }
        }
    }
    lines.join("\n")
}
