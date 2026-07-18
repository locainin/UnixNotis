//! Privacy-safe text and path handling for doctor reports

use std::env;
use std::path::{Path, PathBuf};

use unixnotis_core::util::sanitize_inline_display_text;

pub(in crate::doctor) const DOCTOR_DETAIL_CHAR_LIMIT: usize = 1_024;

pub(in crate::doctor) fn safe_doctor_text(value: &str) -> String {
    // Remove complete terminal sequences before generic control filtering
    // Dropping only ESC would leave visible color parameters such as `[31m`
    let without_terminal_sequences = strip_terminal_sequences(value);
    // Sanitization runs before redaction so control bytes cannot hide a home-path boundary
    let sanitized = sanitize_inline_display_text(&without_terminal_sequences);
    let redacted = redact_home_text(&sanitized);
    truncate_with_ellipsis(&redacted, DOCTOR_DETAIL_CHAR_LIMIT)
}

fn strip_terminal_sequences(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\u{1b}' => {
                // CSI sequences end at an ASCII final byte
                if chars.next_if_eq(&'[').is_some() {
                    for next in chars.by_ref() {
                        if ('@'..='~').contains(&next) {
                            break;
                        }
                    }
                } else if chars.next_if_eq(&']').is_some() {
                    // OSC sequences end at BEL or the ESC-backslash string terminator
                    while let Some(next) = chars.next() {
                        if next == '\u{7}' {
                            break;
                        }
                        if next == '\u{1b}' && chars.next_if_eq(&'\\').is_some() {
                            break;
                        }
                    }
                } else {
                    // Other escape families carry one command byte
                    let _ = chars.next();
                }
            }
            '\u{9b}' => {
                // C1 CSI is the single-codepoint form of ESC-left-bracket
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
            _ => output.push(ch),
        }
    }
    output
}

pub(in crate::doctor) fn redact_home(path: &Path) -> String {
    let Some(home) = home_path() else {
        // Reports remain useful on unusual environments without inventing a home path
        return path.display().to_string();
    };
    let Ok(relative) = path.strip_prefix(&home) else {
        // Non-home paths are preserved because they may be required to diagnose overrides
        return path.display().to_string();
    };
    if relative.as_os_str().is_empty() {
        return "$HOME".to_string();
    }
    format!("$HOME/{}", relative.display())
}

pub(in crate::doctor) fn redact_home_text(value: &str) -> String {
    let Some(home) = home_path() else {
        return value.to_string();
    };
    let Some(home) = home.to_str().filter(|home| !home.is_empty()) else {
        // Non-UTF-8 paths cannot be present inside an external UTF-8 detail string
        return value.to_string();
    };

    let mut redacted = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(index) = remaining.find(home) {
        // Split first so every branch advances without hand-built byte arithmetic
        let (before, matched) = remaining.split_at(index);
        let after = matched
            .strip_prefix(home)
            .expect("matched home prefix should remain present");
        // A path boundary avoids corrupting a longer account name with the same prefix
        let boundary_after = after.is_empty() || after.starts_with('/');
        redacted.push_str(before);
        if boundary_after {
            redacted.push_str("$HOME");
        } else {
            // Preserve a longer account-name prefix while still advancing past this match
            redacted.push_str(home);
        }
        remaining = after;
    }
    redacted.push_str(remaining);
    redacted
}

pub(in crate::doctor) fn truncate_with_ellipsis(value: &str, limit: usize) -> String {
    if limit == 0 {
        // A zero budget must not grow into a one-character ellipsis
        return String::new();
    }
    if value.chars().count() <= limit {
        return value.to_string();
    }
    // Reserve one character so the marker never exceeds the requested limit
    let retained = limit.saturating_sub(1);
    let mut output = value.chars().take(retained).collect::<String>();
    output.push('…');
    output
}

fn home_path() -> Option<PathBuf> {
    env::var_os("HOME")
        // Empty HOME has no safe path boundary and is treated as unavailable
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
#[path = "tests/text.rs"]
mod tests;
