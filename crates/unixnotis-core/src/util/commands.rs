//! Command-shape checks shared by configuration consumers

pub const SHELL_META_CHARS: [char; 15] = [
    '|', '&', ';', '<', '>', '$', '`', '(', ')', '{', '}', '[', ']', '*', '?',
];

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

    // Leading assignments need shell parsing unless the first token is an explicit path
    let first = cmd.split_whitespace().next().unwrap_or_default();
    if first.contains('=') && !first.starts_with('/') && !first.starts_with("./") {
        return false;
    }

    true
}

#[cfg(test)]
#[path = "tests/commands.rs"]
mod tests;
