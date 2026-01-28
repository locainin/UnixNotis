//! Command parsing and heuristics for widget command planning.
//!
//! Keeps shell parsing and "slow command" classification localized so the
//! enqueue/worker pipeline can stay focused on execution and backpressure.

use glib::shell_parse_argv;
use unixnotis_core::util;

pub(super) fn parse_simple_command(cmd: &str) -> Option<(String, Vec<String>)> {
    let cmd = cmd.trim();
    if cmd.is_empty() || !util::is_simple_command(cmd) {
        return None;
    }
    // Use GLib parsing to honor quoted arguments without invoking a shell.
    let mut parts = shell_parse_argv(cmd).ok()?.into_iter();
    let program = parts.next()?.into_string().ok()?;
    let args = parts
        .map(|arg| arg.into_string().ok())
        .collect::<Option<Vec<_>>>()?;
    Some((program, args))
}

pub(super) fn is_probably_slow(cmd: &str) -> bool {
    // Complex commands (shell meta, env assignments, etc.) are treated as slow to
    // avoid under-budgeting timeouts for shells and pipelines.
    let Some((program, args)) = parse_simple_command(cmd) else {
        return true;
    };

    let program_name = program
        .rsplit('/')
        .next()
        .unwrap_or(program.as_str())
        .to_ascii_lowercase();

    if program_name == "sleep" {
        return true;
    }

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
        if let Some(script) = shell_script_arg(&args) {
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
        let (program, args) =
            parse_simple_command("notify-send \"Hello World\"").expect("parsed command");
        assert_eq!(program, "notify-send");
        assert_eq!(args, vec!["Hello World"]);
    }

    #[test]
    fn parse_simple_command_rejects_shell_meta() {
        assert!(parse_simple_command("echo hi | wc -l").is_none());
    }

    #[test]
    fn is_probably_slow_respects_program_tokens() {
        assert!(is_probably_slow("sleep 1"));
        assert!(is_probably_slow("nmcli radio wifi"));
        assert!(!is_probably_slow("echo \"I am not sleeping\""));
    }

    #[test]
    fn is_probably_slow_handles_shell_sleep_script() {
        assert!(is_probably_slow("bash -c \"sleep 1\""));
        assert!(!is_probably_slow("bash -c \"echo sleep\""));
    }
}
