//! Command parsing and heuristics for widget command planning.
//!
//! Keeps shell parsing and "slow command" classification localized so the
//! enqueue/worker pipeline can stay focused on execution and backpressure.

use glib::shell_parse_argv;
use unixnotis_core::util::SHELL_META_CHARS;

pub(super) struct ParsedCommand {
    pub(super) env: Vec<(String, String)>,
    pub(super) program: String,
    pub(super) args: Vec<String>,
}

type EnvAssignments = Vec<(String, String)>;

pub(super) fn parse_simple_command(cmd: &str) -> Option<ParsedCommand> {
    // Simple commands are parsed without a shell so quoting works but
    // shell metacharacters still keep the command on the shell path
    let cmd = cmd.trim();
    if cmd.is_empty() || !is_shell_free_command(cmd) {
        return None;
    }
    // Use GLib parsing to honor quoted arguments without invoking a shell.
    // Parsing failures are treated as non-simple commands and routed through shell mode
    let parts = shell_parse_argv(cmd)
        .ok()?
        .into_iter()
        .map(|part| part.into_string().ok())
        .collect::<Option<Vec<_>>>()?;
    let (env, remaining) = split_leading_env_assignments(parts)?;
    let mut parts = remaining.into_iter();
    let program = parts.next()?;
    let args = parts.collect::<Vec<_>>();
    Some(ParsedCommand { env, program, args })
}

fn is_shell_free_command(cmd: &str) -> bool {
    // This local check stays looser than unixnotis_core::util::is_simple_command because
    // leading NAME=value pairs are safe to apply directly to a child process environment
    if cmd
        .chars()
        .any(|ch| SHELL_META_CHARS.contains(&ch) || ch == '~' || ch == '\n' || ch == '\r')
    {
        return false;
    }
    true
}

fn split_leading_env_assignments(parts: Vec<String>) -> Option<(EnvAssignments, Vec<String>)> {
    let mut env = Vec::new();
    let mut index = 0usize;

    // Only the leading NAME=value segment is treated as process-local environment
    while let Some(part) = parts.get(index) {
        let Some((name, value)) = split_env_assignment(part) else {
            break;
        };
        env.push((name.to_string(), value.to_string()));
        index += 1;
    }

    let remaining = parts.get(index..)?.to_vec();
    if remaining.is_empty() {
        return None;
    }
    Some((env, remaining))
}

fn split_env_assignment(token: &str) -> Option<(&str, &str)> {
    let (name, value) = token.split_once('=')?;
    // Keep assignment parsing strict so malformed shell syntax falls back to sh -c
    if name.is_empty() {
        return None;
    }
    let mut chars = name.chars();
    let first = chars.next()?;
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return None;
    }
    if chars.any(|ch| !(ch == '_' || ch.is_ascii_alphanumeric())) {
        return None;
    }
    Some((name, value))
}

pub(super) fn is_probably_slow(cmd: &str) -> bool {
    // Complex commands (shell meta, unsupported env forms, etc.) are treated as slow to
    // avoid under-budgeting timeouts for shells and pipelines
    let Some(parsed) = parse_simple_command(cmd) else {
        return true;
    };

    // Compare only executable basename so absolute paths and wrappers still match
    let program_name = parsed
        .program
        .rsplit('/')
        .next()
        .unwrap_or(parsed.program.as_str())
        .to_ascii_lowercase();

    if program_name == "sleep" {
        return true;
    }

    // Known utilities that are likely to block or hit D-Bus
    const SLOW_TOKENS: [&str; 9] = [
        "nmcli",
        "bluetoothctl",
        "rfkill",
        "udevadm",
        "upower",
        "playerctl",
        "pactl",
        "wpctl",
        "brightnessctl",
    ];
    if SLOW_TOKENS.contains(&program_name.as_str()) {
        return true;
    }

    if matches!(program_name.as_str(), "sh" | "bash" | "zsh" | "fish") {
        // Shell scripts are treated as slow if the first token is "sleep"
        if let Some(script) = shell_script_arg(&parsed.args) {
            if script.split_whitespace().next() == Some("sleep") {
                return true;
            }
        }
    }

    false
}

fn shell_script_arg(args: &[String]) -> Option<&str> {
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        if arg == "-c" {
            return iter.peek().map(|value| value.as_str());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{is_probably_slow, parse_simple_command};

    #[test]
    fn parse_simple_command_honors_quotes() {
        let parsed = parse_simple_command("notify-send \"Hello World\"").expect("parsed command");
        assert_eq!(parsed.program, "notify-send");
        assert!(parsed.env.is_empty());
        assert_eq!(parsed.args, vec!["Hello World"]);
    }

    #[test]
    fn parse_simple_command_rejects_shell_meta() {
        assert!(parse_simple_command("echo hi | wc -l").is_none());
    }

    #[test]
    fn parse_simple_command_accepts_leading_env_assignments() {
        let parsed =
            parse_simple_command("FOO=bar BAR='two words' notify-send done").expect("parsed");

        assert_eq!(parsed.program, "notify-send");
        assert_eq!(
            parsed.env,
            vec![
                ("FOO".to_string(), "bar".to_string()),
                ("BAR".to_string(), "two words".to_string())
            ]
        );
        assert_eq!(parsed.args, vec!["done"]);
    }

    #[test]
    fn is_probably_slow_respects_program_tokens() {
        assert!(is_probably_slow("sleep 1"));
        assert!(is_probably_slow("nmcli radio wifi"));
        assert!(!is_probably_slow("FOO=bar echo ok"));
        assert!(!is_probably_slow("echo \"I am not sleeping\""));
    }

    #[test]
    fn is_probably_slow_handles_shell_sleep_script() {
        assert!(is_probably_slow("bash -c \"sleep 1\""));
        assert!(!is_probably_slow("bash -c \"echo sleep\""));
    }
}
