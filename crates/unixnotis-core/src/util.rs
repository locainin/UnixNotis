//! Shared helper utilities used across `UnixNotis` components

use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

struct ProgramCache {
    // Snapshot of PATH used to invalidate cached entries when environment changes
    path: Option<String>,
    // Cached program presence results keyed by program name
    results: HashMap<String, bool>,
}

static PROGRAM_CACHE: OnceLock<Mutex<ProgramCache>> = OnceLock::new();
pub const SHELL_META_CHARS: [char; 15] = [
    '|', '&', ';', '<', '>', '$', '`', '(', ')', '{', '}', '[', ']', '*', '?',
];
pub const CONFIG_PATH_ENV: &str = "UNIXNOTIS_CONFIG_PATH";
pub const TRUSTED_SYSTEM_TOOL_DIRS: [&str; 4] = ["/usr/bin", "/bin", "/usr/sbin", "/sbin"];
const DEFAULT_LOG_LIMIT: usize = 160;
const DIAGNOSTIC_LOG_LIMIT: usize = 512;

/// Check whether a program exists in $PATH, caching results to avoid repeated scans
pub fn program_in_path(program: &str) -> bool {
    if program.contains(std::path::MAIN_SEPARATOR) {
        return is_executable_path(Path::new(program));
    }
    // Capture PATH once per call to avoid repeated env lookups
    let current_path = env::var("PATH").ok();
    let cache = PROGRAM_CACHE.get_or_init(|| {
        Mutex::new(ProgramCache {
            path: None,
            results: HashMap::new(),
        })
    });
    if let Ok(mut cache) = cache.lock() {
        // Reset cached lookups whenever PATH changes to avoid stale results in long-lived sessions
        if cache.path.as_deref() != current_path.as_deref() {
            cache.path = current_path.clone();
            cache.results.clear();
        }
        if let Some(result) = cache.results.get(program) {
            return *result;
        }

        let found = current_path.as_ref().is_some_and(|paths| {
            env::split_paths(paths).any(|dir| is_executable_path(&dir.join(program)))
        });

        cache.results.insert(program.to_string(), found);
        return found;
    }

    current_path.as_ref().is_some_and(|paths| {
        env::split_paths(paths).any(|dir| is_executable_path(&dir.join(program)))
    })
}

#[must_use]
pub fn trusted_system_program_path(program: &str) -> Option<PathBuf> {
    if program.is_empty() || program.contains(std::path::MAIN_SEPARATOR) {
        return None;
    }

    // Security-sensitive helpers use an explicit FHS policy instead of attacker-controlled PATH
    TRUSTED_SYSTEM_TOOL_DIRS
        .iter()
        .map(|directory| Path::new(directory).join(program))
        .find(|path| is_executable_path(path))
}

/// Resolve `XDG_STATE_HOME` with the specification defaults
#[must_use]
pub fn resolve_state_dir() -> Option<PathBuf> {
    resolve_state_dir_from_env(
        env::var("XDG_STATE_HOME").ok().as_deref(),
        env::var("HOME").ok().as_deref(),
    )
}

/// Resolve the state directory from explicit environment values
#[must_use]
pub fn resolve_state_dir_from_env(
    xdg_state_home: Option<&str>,
    home: Option<&str>,
) -> Option<PathBuf> {
    if let Some(dir) = xdg_state_home {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            let path = PathBuf::from(trimmed);
            if path.is_absolute() {
                return Some(path);
            }
        }
    }
    let home = home?;
    let trimmed = home.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return None;
    }
    Some(path.join(".local").join("state"))
}

fn is_executable_path(path: &Path) -> bool {
    // Ensure backend selection only succeeds when the program can actually be executed
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Expand leading `~`/`~/` to $HOME, preserving other paths as-is
#[must_use]
pub fn expand_tilde(value: &str) -> Cow<'_, str> {
    let trimmed = value.trim();
    if trimmed == "~" || trimmed.starts_with("~/") {
        if let Ok(home) = env::var("HOME") {
            if trimmed == "~" {
                return home.into();
            }
            let suffix = trimmed.trim_start_matches("~/");
            return format!("{home}/{suffix}").into();
        }
    }
    value.into()
}

/// Returns true when the command can run without a shell wrapper
///
/// # Example
/// ```
/// use unixnotis_core::util::is_simple_command;
///
/// assert!(is_simple_command("echo hello"));
/// assert!(!is_simple_command("echo hello | wc -l"));
/// ```
#[must_use]
pub fn is_simple_command(cmd: &str) -> bool {
    if cmd
        .chars()
        .any(|ch| SHELL_META_CHARS.contains(&ch) || ch == '~' || ch == '\n' || ch == '\r')
    {
        return false;
    }

    let first = cmd.split_whitespace().next().unwrap_or_default();
    if first.contains('=') && !first.starts_with('/') && !first.starts_with("./") {
        return false;
    }

    true
}

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

/// Sanitizes a log string by stripping newlines and capping length
#[must_use]
pub fn sanitize_log_value(value: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }
    // Pre-allocate to reduce churn when sanitizing frequent log values
    let mut cleaned = String::with_capacity(max_len.min(value.len()));
    let mut count = 0usize;
    let mut truncated = false;
    for ch in value.chars() {
        // Directionality controls can visually reorder terminal output, so drop them
        if is_bidi_control(ch) {
            continue;
        }
        // Replace control/newline bytes with spaces to keep logs single-line and safe
        let ch = if ch.is_control() { ' ' } else { ch };
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

/// Produces a safe log snippet honoring diagnostic mode limits
#[must_use]
pub fn log_snippet(value: &str) -> String {
    sanitize_log_value(value, log_limit())
}

/// Sanitizes text that will be shown to the user inside the UI
#[must_use]
pub fn sanitize_display_text(value: &str) -> String {
    // Keep line breaks here
    sanitize_display_text_with(value, true)
}

/// Sanitizes text that must remain single-line and safe for display
#[must_use]
pub fn sanitize_inline_display_text(value: &str) -> String {
    // Keep inline text on one line
    sanitize_display_text_with(value, false)
}

fn sanitize_display_text_with(value: &str, keep_newlines: bool) -> String {
    // Start with the same size
    let mut cleaned = String::with_capacity(value.len());
    for ch in value.chars() {
        // Directionality controls can visually spoof filenames and message text
        if is_bidi_control(ch) {
            continue;
        }

        let mapped = match ch {
            // Keep newlines only when asked
            '\n' if keep_newlines => '\n',
            // Flatten control behavior
            _ if ch.is_control() => ' ',
            _ => ch,
        };
        cleaned.push(mapped);
    }
    cleaned
}

const fn is_bidi_control(ch: char) -> bool {
    // Covers directional embeddings/overrides/isolates and directional marks
    matches!(
        ch,
        '\u{061C}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'
            | '\u{202B}'
            | '\u{202C}'
            | '\u{202D}'
            | '\u{202E}'
            | '\u{2066}'
            | '\u{2067}'
            | '\u{2068}'
            | '\u{2069}'
    )
}

#[cfg(test)]
#[path = "tests/util/commands.rs"]
mod command_tests;
#[cfg(test)]
#[path = "tests/util/diagnostics.rs"]
mod diagnostic_tests;
#[cfg(test)]
#[path = "tests/util/display.rs"]
mod display_tests;
#[cfg(test)]
#[path = "tests/util/paths.rs"]
mod path_tests;
#[cfg(test)]
#[path = "tests/util/programs.rs"]
mod program_tests;
