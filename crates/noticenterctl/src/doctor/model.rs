//! Stable report types shared by human and JSON doctor output

use serde::Serialize;

pub(super) const DOCTOR_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DoctorSeverity {
    // Ordering grows with operational impact
    Pass,
    Note,
    Warning,
    Error,
}

impl DoctorSeverity {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Note => "note",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct DoctorCheck {
    pub id: String,
    pub label: String,
    pub severity: DoctorSeverity,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl DoctorCheck {
    pub(super) fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        severity: DoctorSeverity,
        summary: impl Into<String>,
    ) -> Self {
        // Required fields keep every check useful in both renderers
        Self {
            id: id.into(),
            label: label.into(),
            severity,
            summary: summary.into(),
            details: None,
            hint: None,
        }
    }

    pub(super) fn details(mut self, details: impl Into<String>) -> Self {
        // Builder methods keep check construction readable at call sites
        self.details = Some(details.into());
        self
    }

    pub(super) fn hint(mut self, hint: impl Into<String>) -> Self {
        // Hints remain optional so successful checks stay compact
        self.hint = Some(hint.into());
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DoctorLogSource {
    SystemdJournal,
    Dinit,
    Runit,
    S6Rc,
    Manual,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(super) enum DoctorLogResult {
    // The explicit tag keeps JSON consumers independent of field presence
    Collected {
        source: DoctorLogSource,
        lines: Vec<String>,
    },
    Unavailable {
        source: DoctorLogSource,
        reason: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct DoctorReport {
    pub schema_version: u32,
    pub unixnotis_version: String,
    pub checks: Vec<DoctorCheck>,
    pub logs: DoctorLogResult,
}

impl DoctorReport {
    pub(super) fn new(checks: Vec<DoctorCheck>, logs: DoctorLogResult) -> Self {
        // Schema version changes only when the serialized contract changes
        Self {
            schema_version: DOCTOR_SCHEMA_VERSION,
            unixnotis_version: env!("CARGO_PKG_VERSION").to_string(),
            checks,
            logs,
        }
    }

    pub(super) fn has_errors(&self) -> bool {
        // Notes and warnings remain successful diagnostic outcomes
        // A single objective error controls the process exit after all checks finish
        self.checks
            .iter()
            .any(|check| check.severity == DoctorSeverity::Error)
    }
}

#[cfg(test)]
#[path = "tests/model.rs"]
mod tests;
