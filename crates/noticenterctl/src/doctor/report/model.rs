//! Stable report types shared by human and JSON doctor output

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;
use unixnotis_core::ConfigDiagnostic;

pub(super) const DOCTOR_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::doctor) enum DoctorSeverity {
    // Ordering grows with operational impact
    Pass,
    Note,
    Warning,
    Error,
}

impl DoctorSeverity {
    pub(in crate::doctor) const fn label(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Note => "note",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(in crate::doctor) struct DoctorCheck {
    pub id: String,
    pub label: String,
    pub severity: DoctorSeverity,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub data: BTreeMap<String, Value>,
}

impl DoctorCheck {
    pub(in crate::doctor) fn new(
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
            data: BTreeMap::new(),
        }
    }

    pub(in crate::doctor) fn details(mut self, details: impl Into<String>) -> Self {
        // Builder methods keep check construction readable at call sites
        self.details = Some(details.into());
        self
    }

    pub(in crate::doctor) fn hint(mut self, hint: impl Into<String>) -> Self {
        // Hints remain optional so successful checks stay compact
        self.hint = Some(hint.into());
        self
    }

    pub(in crate::doctor) fn data(
        mut self,
        key: impl Into<String>,
        value: impl Into<Value>,
    ) -> Self {
        // Stable typed fields keep JSON consumers independent from human detail formatting
        self.data.insert(key.into(), value.into());
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::doctor) enum DoctorLogSource {
    SystemdJournal,
    Dinit,
    Runit,
    S6Rc,
    Manual,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(in crate::doctor) enum DoctorLogResult {
    // The explicit tag keeps JSON consumers independent of field presence
    Collected {
        source: DoctorLogSource,
        lines: Vec<String>,
        truncated: bool,
        line_limit: usize,
        byte_limit: usize,
    },
    Unavailable {
        source: DoctorLogSource,
        reason: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(in crate::doctor) struct DoctorReport {
    pub schema_version: u32,
    pub unixnotis_version: String,
    pub checks: Vec<DoctorCheck>,
    pub config_diagnostics: Vec<ConfigDiagnostic>,
    pub logs: DoctorLogResult,
}

impl DoctorReport {
    pub(in crate::doctor) fn new(
        checks: Vec<DoctorCheck>,
        config_diagnostics: Vec<ConfigDiagnostic>,
        logs: DoctorLogResult,
    ) -> Self {
        // Schema version changes only when the serialized contract changes
        Self {
            schema_version: DOCTOR_SCHEMA_VERSION,
            unixnotis_version: env!("CARGO_PKG_VERSION").to_string(),
            checks,
            config_diagnostics,
            logs,
        }
    }

    pub(in crate::doctor) fn has_errors(&self) -> bool {
        // Notes and warnings remain successful diagnostic outcomes
        // A single objective error controls the process exit after all checks finish
        self.checks
            .iter()
            .any(|check| check.severity == DoctorSeverity::Error)
    }
}
