use std::path::Path;

/// Convert an env value into envdir file contents
///
/// Envdir readers only use the first line and trim trailing blanks. Matching
/// that behavior before writing avoids keeping stale shell noise
pub(in crate::service_manager) fn envdir_file_contents(value: Option<&str>) -> String {
    unixnotis_core::service_manager::envdir_file_contents(value)
}

/// Return true when a variable name can safely become an envdir file name
pub(in crate::service_manager) fn is_safe_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    // Keep names in ordinary environment-variable form so they remain safe file names
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

pub(in crate::service_manager) fn shell_quote_path(path: &Path) -> String {
    shell_quote(&path.display().to_string())
}

pub(in crate::service_manager) fn shell_quote(raw: &str) -> String {
    if raw.is_empty() {
        return "''".to_string();
    }

    let mut quoted = String::with_capacity(raw.len() + 2);
    quoted.push('\'');
    for ch in raw.chars() {
        if ch == '\'' {
            // POSIX single-quote escape: close, emit escaped quote, reopen
            quoted.push_str("'\\''");
        } else {
            quoted.push(ch);
        }
    }
    quoted.push('\'');
    quoted
}
