//! Shared helper utilities used across UnixNotis components.

use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

struct ProgramCache {
    // Snapshot of PATH used to invalidate cached entries when environment changes.
    path: Option<String>,
    // Cached program presence results keyed by program name.
    results: HashMap<String, bool>,
}

static PROGRAM_CACHE: OnceLock<Mutex<ProgramCache>> = OnceLock::new();
const DEFAULT_LOG_LIMIT: usize = 160;
const DIAGNOSTIC_LOG_LIMIT: usize = 512;

/// Check whether a program exists in $PATH, caching results to avoid repeated scans.
pub fn program_in_path(program: &str) -> bool {
    if program.contains(std::path::MAIN_SEPARATOR) {
        return Path::new(program).is_file();
    }
    // Capture PATH once per call to avoid repeated env lookups.
    let current_path = env::var("PATH").ok();
    let cache = PROGRAM_CACHE.get_or_init(|| {
        Mutex::new(ProgramCache {
            path: None,
            results: HashMap::new(),
        })
    });
    if let Ok(mut cache) = cache.lock() {
        // Reset cached lookups whenever PATH changes to avoid stale results in long-lived sessions.
        if cache.path.as_deref() != current_path.as_deref() {
            cache.path = current_path.clone();
            cache.results.clear();
        }
        if let Some(result) = cache.results.get(program) {
            return *result;
        }
    }

    let found = current_path
        .as_ref()
        .map(|paths| env::split_paths(paths).any(|dir| dir.join(program).is_file()))
        .unwrap_or(false);

    if let Ok(mut cache) = cache.lock() {
        if cache.path.as_deref() != current_path.as_deref() {
            cache.path = current_path.clone();
            cache.results.clear();
        }
        cache.results.insert(program.to_string(), found);
    }

    found
}

/// Returns true when diagnostics are explicitly enabled via environment.
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
    // Pre-allocate to reduce churn when sanitizing frequent log values.
    let mut cleaned = String::with_capacity(max_len.min(value.len()));
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
