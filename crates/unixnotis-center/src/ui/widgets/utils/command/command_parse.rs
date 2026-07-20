//! Typed command heuristics for widget execution planning
//!
//! Keeps shell parsing and "slow command" classification localized so the
//! enqueue/worker pipeline can stay focused on execution and backpressure

use std::ffi::OsStr;

use unixnotis_core::CommandSpec;

pub(super) fn is_probably_slow(cmd: &CommandSpec) -> bool {
    let CommandSpec::Direct { program, args, .. } = cmd else {
        return true;
    };

    // Compare only executable basename so absolute paths and wrappers still match
    let program_name = program
        .file_name()
        .unwrap_or(program.as_os_str())
        .to_string_lossy()
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
        if let Some(script) = shell_script_arg(args) {
            if script.split_whitespace().next() == Some("sleep") {
                return true;
            }
        }
    }

    false
}

fn shell_script_arg(args: &[std::ffi::OsString]) -> Option<&str> {
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        if arg == OsStr::new("-c") {
            return iter.peek().and_then(|value| value.to_str());
        }
    }
    None
}

#[cfg(test)]
#[path = "tests/command_parse.rs"]
mod tests;
