//! Configuration path resolution and acceptance checks

use std::path::PathBuf;

use unixnotis_core::{Config, ConfigDiagnostic, ConfigLoadReport, CURRENT_CONFIG_VERSION};

use super::super::report::{redact_home, safe_doctor_text};
use super::super::report::{DoctorCheck, DoctorSeverity};
pub(super) use crate::config_path::{resolve_config_path, ConfigPathSource};

pub(in crate::doctor) struct DoctorConfigResult {
    pub config_path: PathBuf,
    pub report: Option<ConfigLoadReport>,
    pub diagnostics: Vec<ConfigDiagnostic>,
    pub checks: Vec<DoctorCheck>,
}

pub(in crate::doctor) fn inspect_config(requested_path: Option<PathBuf>) -> DoctorConfigResult {
    let mut checks = Vec::new();
    // Resolve once so every later check describes the exact same file
    let (config_path, mut source) = match resolve_config_path(requested_path) {
        Ok(resolved) => resolved,
        Err(error) => {
            checks.push(
                DoctorCheck::new(
                    "environment.config-path",
                    "Environment",
                    DoctorSeverity::Error,
                    "Unable to resolve the doctor configuration path",
                )
                .details(safe_doctor_text(&error.to_string())),
            );
            return DoctorConfigResult {
                config_path: PathBuf::new(),
                report: None,
                diagnostics: Vec::new(),
                checks,
            };
        }
    };
    // Reports should remain shareable without exposing the account home path
    let display_path = redact_home(&config_path);
    checks.push(
        DoctorCheck::new(
            "environment.config-path",
            "Environment",
            DoctorSeverity::Pass,
            "Doctor-resolved configuration path selected",
        )
        .details(display_path.clone())
        .data("path", display_path.clone())
        .data(
            "source",
            serde_json::to_value(source).expect("config path source must serialize"),
        ),
    );

    // Existing files use the same accepted load path as the running processes
    let report = if config_path.exists() {
        match Config::load_from_path_with_report(&config_path) {
            Ok(report) => Some(report),
            Err(error) => {
                checks.push(
                    DoctorCheck::new(
                        "config.acceptance",
                        "Configuration",
                        DoctorSeverity::Error,
                        "Configuration was rejected",
                    )
                    .details(error.shareable_summary())
                    .hint("Correct config.toml, then run noticenterctl doctor again"),
                );
                None
            }
        }
    } else if source.is_explicit() {
        // An explicit missing path is a real setup error rather than a default request
        checks.push(
            DoctorCheck::new(
                "config.acceptance",
                "Configuration",
                DoctorSeverity::Error,
                "Explicit configuration file does not exist",
            )
            .details(display_path),
        );
        None
    } else {
        // A first launch without a file intentionally uses the embedded defaults
        match Config::load_default_with_report() {
            Ok(report) => {
                source = ConfigPathSource::Builtin;
                checks.push(DoctorCheck::new(
                    "config.default",
                    "Configuration",
                    DoctorSeverity::Note,
                    "No config.toml exists, so built-in defaults are active",
                ));
                Some(report)
            }
            Err(error) => {
                checks.push(
                    DoctorCheck::new(
                        "config.acceptance",
                        "Configuration",
                        DoctorSeverity::Error,
                        "Built-in configuration could not be prepared",
                    )
                    .details(error.shareable_summary()),
                );
                None
            }
        }
    };

    if let Some(report) = &report {
        // Acceptance and schema checks stay separate for stable machine-readable IDs
        checks.push(
            DoctorCheck::new(
                "config.acceptance",
                "Configuration",
                DoctorSeverity::Pass,
                "Configuration was accepted",
            )
            .details(format!("Schema version {}", report.config.config_version))
            .data("schema_version", report.config.config_version),
        );
        checks.push(DoctorCheck::new(
            "config.schema",
            "Configuration schema",
            DoctorSeverity::Pass,
            format!("Current schema version {CURRENT_CONFIG_VERSION} is active"),
        ));
    }

    // Structured diagnostics stay separate from checks for stable JSON consumers
    let diagnostics = report
        .as_ref()
        .map_or_else(Vec::new, |report| report.diagnostics.clone());
    // Built-in fallback is known only after the default load succeeds
    if source == ConfigPathSource::Builtin {
        if let Some(check) = checks
            .iter_mut()
            .find(|check| check.id == "environment.config-path")
        {
            check.data.insert(
                "source".to_string(),
                serde_json::to_value(source).expect("config path source must serialize"),
            );
        }
    }
    DoctorConfigResult {
        config_path,
        report,
        diagnostics,
        checks,
    }
}
