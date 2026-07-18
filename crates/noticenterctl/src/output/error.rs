//! Bounded terminal-safe rendering for top-level CLI failures

use unixnotis_core::util;

const MAX_CLI_ERROR_CHARS: usize = 4_096;

pub fn format_cli_error(error: &anyhow::Error) -> String {
    // Alternate formatting keeps the error chain while the sanitizer flattens hostile controls
    let rendered = format!("{error:#}");
    let safe = util::sanitize_log_value(&rendered, MAX_CLI_ERROR_CHARS);
    format!("Error: {safe}\n")
}

#[cfg(test)]
#[path = "tests/error.rs"]
mod tests;
