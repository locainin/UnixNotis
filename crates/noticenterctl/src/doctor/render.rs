//! Deterministic doctor report rendering

use anyhow::Result;

use super::model::{DoctorLogResult, DoctorReport};

pub(super) fn render_json(report: &DoctorReport) -> Result<String> {
    // Pretty JSON remains readable as an issue attachment while preserving the schema
    Ok(serde_json::to_string_pretty(report)?)
}

pub(super) fn render_human(report: &DoctorReport) -> String {
    // The heading includes both software and report schema versions
    let mut lines = vec![format!(
        "UnixNotis doctor {} (schema {})",
        report.unixnotis_version, report.schema_version
    )];

    // Input order is retained so related checks stay grouped predictably
    for check in &report.checks {
        lines.push(String::new());
        lines.push(check.label.to_uppercase());
        lines.push(format!("[{}] {}", check.severity.label(), check.summary));
        // Optional context stays on plain lines for easy terminal copying
        if let Some(details) = &check.details {
            lines.push(details.clone());
        }
        if let Some(hint) = &check.hint {
            lines.push(format!("Hint: {hint}"));
        }
    }

    lines.push(String::new());
    lines.push("LOGS".to_string());
    // Tagged log outcomes map to one compact human section
    match &report.logs {
        DoctorLogResult::Collected {
            source,
            lines: logs,
        } => {
            // Indentation distinguishes collected log lines from report fields
            lines.push(format!("Source: {source:?}"));
            lines.extend(logs.iter().map(|line| format!("  {line}")));
        }
        DoctorLogResult::Unavailable { reason, hint, .. } => {
            // Unavailable sources explain the limitation without pretending collection failed
            lines.push("Persistent logs: unavailable".to_string());
            lines.push(reason.clone());
            if let Some(hint) = hint {
                lines.push(format!("Hint: {hint}"));
            }
        }
    }
    lines.join("\n")
}

#[cfg(test)]
#[path = "tests/render.rs"]
mod tests;
