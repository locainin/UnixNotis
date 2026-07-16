//! Structured configuration load diagnostics

use std::collections::BTreeSet;

use serde::Serialize;
use toml::Value;
use tracing::{info, warn};

use super::{Config, CURRENT_CONFIG_VERSION};

/// Classification used by configuration diagnostics and doctor output
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigDiagnosticKind {
    /// Extra context that does not indicate a problem
    Note,
    /// Accepted input that is suspicious or was ignored
    Warning,
    /// A value changed before reaching runtime code
    Adjustment,
}

/// One stable explanation of how configuration input was interpreted
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConfigDiagnostic {
    /// Stable machine-readable identifier
    pub code: &'static str,
    /// Diagnostic classification
    pub kind: ConfigDiagnosticKind,
    /// Dotted configuration key when one value caused the result
    pub path: Option<String>,
    /// Human-readable summary that is safe to share
    pub message: String,
    /// Original non-sensitive scalar value or structural description
    pub original: Option<String>,
    /// Effective non-sensitive scalar value or structural description
    pub effective: Option<String>,
}

/// Accepted configuration plus every warning and runtime adjustment
#[derive(Clone, Debug)]
pub struct ConfigLoadReport {
    /// Effective configuration used by runtime callers
    pub config: Config,
    /// Stable diagnostics produced while accepting the input
    pub diagnostics: Vec<ConfigDiagnostic>,
}

pub(super) fn migration_diagnostic(contents: &str) -> Option<ConfigDiagnostic> {
    // Diagnostics inspect a separate value tree so deserialization behavior stays unchanged
    let document = contents.parse::<Value>().ok()?;
    // Unversioned files are schema zero and follow the explicit legacy migration path
    let version = document
        .as_table()
        .and_then(|root| root.get("config_version"))
        .and_then(Value::as_integer)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0);
    (version < CURRENT_CONFIG_VERSION).then(|| ConfigDiagnostic {
        code: "config.schema.migrated",
        kind: ConfigDiagnosticKind::Note,
        path: Some("config_version".to_string()),
        message: "Configuration was migrated to the current schema".to_string(),
        original: Some(version.to_string()),
        effective: Some(CURRENT_CONFIG_VERSION.to_string()),
    })
}

pub(super) fn migrated_field_diagnostic(path: String) -> ConfigDiagnostic {
    ConfigDiagnostic {
        code: "config.schema.field-migrated",
        kind: ConfigDiagnosticKind::Note,
        path: Some(path),
        message: "Missing legacy field received its schema-compatible value".to_string(),
        original: None,
        effective: None,
    }
}

pub(super) fn unknown_key_diagnostic(path: String) -> ConfigDiagnostic {
    ConfigDiagnostic {
        code: "config.unknown-key",
        kind: ConfigDiagnosticKind::Warning,
        path: Some(path),
        message: "Unknown configuration key was ignored".to_string(),
        original: None,
        effective: None,
    }
}

pub(super) fn adjustment_diagnostics(before: &Config, after: &Config) -> Vec<ConfigDiagnostic> {
    // TOML values provide one generic tree walk across every current and future config section
    let Ok(before) = Value::try_from(before) else {
        return Vec::new();
    };
    let Ok(after) = Value::try_from(after) else {
        return Vec::new();
    };
    let mut diagnostics = Vec::new();
    collect_adjustments("", Some(&before), Some(&after), &mut diagnostics);
    // Stable ordering keeps logs, doctor JSON, and regression fixtures deterministic
    diagnostics.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.code.cmp(right.code))
    });
    diagnostics
}

fn collect_adjustments(
    path: &str,
    before: Option<&Value>,
    after: Option<&Value>,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) {
    if before == after {
        return;
    }
    match (before, after) {
        (Some(Value::Table(before)), Some(Value::Table(after))) => {
            // A sorted set visits keys present on either side exactly once
            let keys = before.keys().chain(after.keys()).collect::<BTreeSet<_>>();
            for key in keys {
                let child = join_path(path, key);
                collect_adjustments(&child, before.get(key), after.get(key), diagnostics);
            }
        }
        (Some(Value::Array(before)), Some(Value::Array(after))) => {
            // Length changes are useful without exposing any array element text
            if before.len() != after.len() {
                diagnostics.push(adjustment(
                    path,
                    Some(format!("{} item(s)", before.len())),
                    Some(format!("{} item(s)", after.len())),
                ));
            }
            // Shared indices still need field-level diagnostics for structured entries
            for index in 0..before.len().min(after.len()) {
                let child = format!("{path}[{index}]");
                collect_adjustments(&child, before.get(index), after.get(index), diagnostics);
            }
        }
        _ => diagnostics.push(adjustment(
            path,
            before.map(safe_value),
            after.map(safe_value),
        )),
    }
}

fn adjustment(path: &str, original: Option<String>, effective: Option<String>) -> ConfigDiagnostic {
    ConfigDiagnostic {
        code: adjustment_code(path),
        kind: ConfigDiagnosticKind::Adjustment,
        path: (!path.is_empty()).then(|| path.to_string()),
        message: adjustment_message(path).to_string(),
        original,
        effective,
    }
}

fn adjustment_code(path: &str) -> &'static str {
    // Specific codes remain stable even when the user-facing wording improves
    if path.starts_with("widgets.volume.")
        && [
            "enabled",
            "get_cmd",
            "set_cmd",
            "toggle_cmd",
            "watch_cmd",
            "parse_mode",
        ]
        .iter()
        .any(|field| path.ends_with(field))
    {
        "config.widgets.volume-backend-selected"
    } else if path == "widgets.brightness.watch_cmd" {
        "config.widgets.brightness-backend-corrected"
    } else if path == "widgets.refresh_interval_ms" || path == "widgets.refresh_interval_slow_ms" {
        "config.widgets.refresh-clamped"
    } else if path == "history.max_entries" || path == "history.max_active" {
        "config.history.limit-clamped"
    } else if path.starts_with("widgets.toggles")
        || path.starts_with("widgets.stats")
        || path.starts_with("widgets.cards")
    {
        if path.ends_with("plugin") {
            "config.widget.plugin-disabled"
        } else {
            "config.widgets.value-adjusted"
        }
    } else if path.starts_with("widgets.volume") || path.starts_with("widgets.brightness") {
        "config.widgets.slider-adjusted"
    } else if path.starts_with("panel") {
        "config.panel.value-adjusted"
    } else if path.starts_with("popups") {
        "config.popups.value-adjusted"
    } else if path.starts_with("media") {
        "config.media.value-adjusted"
    } else if path.starts_with("theme") {
        "config.theme.value-adjusted"
    } else {
        "config.value-adjusted"
    }
}

fn adjustment_message(path: &str) -> &'static str {
    if path.ends_with("plugin") {
        "Invalid widget plugin configuration was disabled"
    } else if path.contains("refresh_interval") {
        "Refresh interval was adjusted to a safe runtime value"
    } else if path.starts_with("history") {
        "History limit was adjusted to a safe runtime value"
    } else if path.starts_with("widgets") {
        "Widget configuration was adjusted before use"
    } else if path.starts_with("panel") {
        "Panel configuration was adjusted before use"
    } else if path.starts_with("popups") {
        "Popup configuration was adjusted before use"
    } else if path.starts_with("media") {
        "Media configuration was adjusted before use"
    } else if path.starts_with("theme") {
        "Theme configuration was adjusted before use"
    } else {
        "Configuration value was adjusted before use"
    }
}

fn safe_value(value: &Value) -> String {
    // Commands, labels, paths, and other strings are represented only by character count
    match value {
        Value::Integer(value) => value.to_string(),
        Value::Float(value) if value.is_finite() => value.to_string(),
        Value::Float(_) => "non-finite number".to_string(),
        Value::Boolean(value) => value.to_string(),
        Value::Datetime(_) => "datetime".to_string(),
        Value::String(value) => format!("text length {}", value.chars().count()),
        Value::Array(value) => format!("{} item(s)", value.len()),
        Value::Table(value) => format!("{} field(s)", value.len()),
    }
}

fn join_path(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_string()
    } else {
        format!("{parent}.{child}")
    }
}

/// Log accepted configuration diagnostics once for compatibility callers
pub fn log_config_diagnostics(diagnostics: &[ConfigDiagnostic]) {
    for diagnostic in diagnostics {
        // Compatibility logs include identity and path but never original or effective text
        let path = diagnostic.path.as_deref().unwrap_or("config");
        match diagnostic.kind {
            ConfigDiagnosticKind::Warning => {
                warn!(code = diagnostic.code, path, "{}", diagnostic.message);
            }
            ConfigDiagnosticKind::Note | ConfigDiagnosticKind::Adjustment => {
                info!(code = diagnostic.code, path, "{}", diagnostic.message);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/diagnostics.rs"]
mod tests;
