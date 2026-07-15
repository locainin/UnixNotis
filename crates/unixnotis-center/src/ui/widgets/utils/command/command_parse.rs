//! Command parsing and heuristics for widget command planning.
//!
//! Keeps shell parsing and "slow command" classification localized so the
//! enqueue/worker pipeline can stay focused on execution and backpressure.

pub(super) use unixnotis_core::ParsedCommand;
use unixnotis_core::{parse_command, ExecutionMode};

pub(super) fn parse_simple_command(cmd: &str) -> Option<ParsedCommand> {
    // Runtime consumes the same parsed representation used by preset security checks
    let parsed = parse_command(cmd).ok()?;
    (parsed.execution_mode == ExecutionMode::Direct).then_some(parsed)
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
#[path = "tests/command_parse.rs"]
mod tests;
