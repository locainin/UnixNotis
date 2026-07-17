//! Diagnostic-mode switches and log snippet limits

use std::env;

use super::display::sanitize_log_value;

const DEFAULT_LOG_LIMIT: usize = 160;
const DIAGNOSTIC_LOG_LIMIT: usize = 512;

/// Returns true when diagnostics are explicitly enabled via environment
#[must_use]
pub fn diagnostic_mode() -> bool {
    diagnostic_mode_from(env::var("UNIXNOTIS_DIAGNOSTIC").ok().as_deref())
}

fn diagnostic_mode_from(value: Option<&str>) -> bool {
    matches!(
        value
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Returns the default redaction length for logs
#[must_use]
pub const fn default_log_limit() -> usize {
    DEFAULT_LOG_LIMIT
}

/// Returns the diagnostic redaction length for logs
#[must_use]
pub const fn diagnostic_log_limit() -> usize {
    DIAGNOSTIC_LOG_LIMIT
}

/// Returns the effective log snippet limit for the current mode
#[must_use]
pub fn log_limit() -> usize {
    log_limit_for(diagnostic_mode())
}

const fn log_limit_for(diagnostic: bool) -> usize {
    if diagnostic {
        diagnostic_log_limit()
    } else {
        default_log_limit()
    }
}

/// Produces a safe log snippet honoring diagnostic mode limits
#[must_use]
pub fn log_snippet(value: &str) -> String {
    sanitize_log_value(value, log_limit())
}

#[cfg(test)]
#[path = "tests/diagnostics.rs"]
mod tests;
