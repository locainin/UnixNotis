//! Shared helper utilities used across UnixNotis components.

use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

static PROGRAM_CACHE: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();
const DEFAULT_LOG_LIMIT: usize = 160;
const DIAGNOSTIC_LOG_LIMIT: usize = 512;

/// Check whether a program exists in $PATH, caching results to avoid repeated scans.
pub fn program_in_path(program: &str) -> bool {
    if program.contains(std::path::MAIN_SEPARATOR) {
        return Path::new(program).is_file();
    }
    let cache = PROGRAM_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(cache) = cache.lock() {
        if let Some(result) = cache.get(program) {
            return *result;
        }
    }

    let found = match env::var("PATH") {
        Ok(paths) => env::split_paths(&paths)
            .any(|dir| dir.join(program).is_file()),
        Err(_) => false,
    };

    if let Ok(mut cache) = cache.lock() {
        cache.insert(program.to_string(), found);
    }

    found
}

/// Returns true when diagnostics are explicitly enabled via environment.
pub fn diagnostic_mode() -> bool {
    diagnostic_mode_from(env::var("UNIXNOTIS_DIAGNOSTIC").ok().as_deref())
}

fn diagnostic_mode_from(value: Option<&str>) -> bool {
    matches!(
        value.unwrap_or_default().trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Returns the default redaction length for logs.
pub fn default_log_limit() -> usize {
    DEFAULT_LOG_LIMIT
}

/// Returns the diagnostic redaction length for logs.
pub fn diagnostic_log_limit() -> usize {
    DIAGNOSTIC_LOG_LIMIT
}

/// Returns the effective log snippet limit for the current mode.
pub fn log_limit() -> usize {
    log_limit_for(diagnostic_mode())
}

fn log_limit_for(diagnostic: bool) -> usize {
    if diagnostic {
        diagnostic_log_limit()
    } else {
        default_log_limit()
    }
}

/// Sanitizes a log string by stripping newlines and capping length.
pub fn sanitize_log_value(value: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }
    let mut cleaned = String::new();
    let mut count = 0usize;
    let mut truncated = false;
    for ch in value.chars() {
        let ch = if ch == '\n' || ch == '\r' { ' ' } else { ch };
        cleaned.push(ch);
        count += 1;
        if count >= max_len {
            truncated = true;
            break;
        }
    }
    let trimmed = cleaned.trim();
    if truncated {
        format!("{trimmed}...")
    } else {
        trimmed.to_string()
    }
}

/// Produces a safe log snippet honoring diagnostic mode limits.
pub fn log_snippet(value: &str) -> String {
    sanitize_log_value(value, log_limit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_log_value_strips_newlines_and_caps() {
        let value = "ab\ncd\rEF";
        let sanitized = sanitize_log_value(value, 5);
        assert_eq!(sanitized, "ab cd...");

        let no_truncate = sanitize_log_value("ok", 5);
        assert_eq!(no_truncate, "ok");
    }

    #[test]
    fn diagnostic_mode_parses_expected_values() {
        assert!(diagnostic_mode_from(Some("1")));
        assert!(diagnostic_mode_from(Some("true")));
        assert!(diagnostic_mode_from(Some("YES")));
        assert!(diagnostic_mode_from(Some("on")));
        assert!(!diagnostic_mode_from(Some("0")));
        assert!(!diagnostic_mode_from(Some("false")));
        assert!(!diagnostic_mode_from(None));
    }

    #[test]
    fn log_limit_respects_mode() {
        assert_eq!(log_limit_for(false), DEFAULT_LOG_LIMIT);
        assert_eq!(log_limit_for(true), DIAGNOSTIC_LOG_LIMIT);
    }
}
