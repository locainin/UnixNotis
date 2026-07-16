//! Configuration path resolution and acceptance checks

use std::env;
use std::path::{Path, PathBuf};

use unixnotis_core::util::CONFIG_PATH_ENV;
use unixnotis_core::{Config, ConfigDiagnosticKind, ConfigLoadReport, CURRENT_CONFIG_VERSION};

use super::model::{DoctorCheck, DoctorSeverity};

pub(super) struct DoctorConfigResult {
    pub config_path: PathBuf,
    pub report: Option<ConfigLoadReport>,
    pub checks: Vec<DoctorCheck>,
}

pub(super) fn inspect_config() -> DoctorConfigResult {
    let mut checks = Vec::new();
    // Resolve once so every later check describes the exact same file
    let config_path = match resolve_config_path() {
        Ok(path) => path,
        Err(error) => {
            checks.push(
                DoctorCheck::new(
                    "environment.config-path",
                    "Environment",
                    DoctorSeverity::Error,
                    "Unable to resolve the active configuration path",
                )
                .details(error.to_string()),
            );
            return DoctorConfigResult {
                config_path: PathBuf::new(),
                report: None,
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
            "Active configuration path resolved",
        )
        .details(display_path.clone()),
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
    } else if explicit_config_path_is_set() {
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
            .details(format!("Schema version {}", report.config.config_version)),
        );
        checks.push(DoctorCheck::new(
            "config.schema",
            "Configuration schema",
            DoctorSeverity::Pass,
            format!("Current schema version {CURRENT_CONFIG_VERSION} is active"),
        ));
        // Preserve parser order so repeated doctor runs produce comparable reports
        checks.extend(
            report
                .diagnostics
                .iter()
                .enumerate()
                .map(|(index, diagnostic)| {
                    // The source index keeps duplicate diagnostic codes uniquely addressable
                    // Adjustments are accepted behavior while ignored keys remain warnings
                    let severity = match diagnostic.kind {
                        ConfigDiagnosticKind::Note | ConfigDiagnosticKind::Adjustment => {
                            DoctorSeverity::Note
                        }
                        ConfigDiagnosticKind::Warning => DoctorSeverity::Warning,
                    };
                    let mut check = DoctorCheck::new(
                        format!("config.diagnostic.{index}.{}", diagnostic.code),
                        "Configuration diagnostic",
                        severity,
                        diagnostic.message.clone(),
                    );
                    // Only pre-redacted scalar details from ConfigDiagnostic are copied here
                    let mut details = Vec::new();
                    if let Some(path) = &diagnostic.path {
                        details.push(format!("Key: {path}"));
                    }
                    if let Some(original) = &diagnostic.original {
                        details.push(format!("Original: {original}"));
                    }
                    if let Some(effective) = &diagnostic.effective {
                        details.push(format!("Effective: {effective}"));
                    }
                    if !details.is_empty() {
                        // Empty detail blocks are omitted from both human and JSON renderers
                        check = check.details(details.join("\n"));
                    }
                    check
                }),
        );
    }

    DoctorConfigResult {
        config_path,
        report,
        checks,
    }
}

pub(super) fn resolve_config_path() -> Result<PathBuf, unixnotis_core::ConfigError> {
    Config::active_config_path()
}

fn explicit_config_path_is_set() -> bool {
    // Empty environment values follow normal default path selection
    env::var_os(CONFIG_PATH_ENV).is_some_and(|value| !value.is_empty())
}

pub(super) fn redact_home(path: &Path) -> String {
    // Prefix matching avoids replacing home-like text in unrelated paths
    let home = env::var_os("HOME").filter(|value| !value.is_empty());
    if let Some(home) = home {
        if let Ok(relative) = path.strip_prefix(PathBuf::from(home)) {
            if relative.as_os_str().is_empty() {
                // The home root has no trailing slash in shareable output
                return "$HOME".to_string();
            }
            return format!("$HOME/{}", relative.display());
        }
    }
    path.display().to_string()
}

pub(super) fn redact_home_text(value: &str) -> String {
    let Some(home) = env::var_os("HOME").filter(|home| !home.is_empty()) else {
        return value.to_string();
    };
    let home = PathBuf::from(home);
    let Some(home) = home.to_str().filter(|home| !home.is_empty()) else {
        // Non-UTF-8 home paths cannot appear inside UTF-8 command output
        return value.to_string();
    };
    // Free-form command output may contain several paths on one line
    value.replace(home, "$HOME")
}

#[cfg(test)]
#[path = "tests/config.rs"]
mod tests;
