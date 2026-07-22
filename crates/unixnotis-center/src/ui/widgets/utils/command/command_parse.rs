//! Typed command heuristics for widget execution planning
//!
//! Keeps shell parsing and "slow command" classification localized so the
//! enqueue/worker pipeline can stay focused on execution and backpressure

use unixnotis_core::CommandSpec;

pub(super) fn is_probably_slow(cmd: &CommandSpec) -> bool {
    // Shared shell detection owns every direct interpreter spelling
    // Shell startup and script execution belong on the wider timeout budget
    if cmd.uses_shell_command_string() {
        return true;
    }

    let CommandSpec::Direct { program, .. } = cmd else {
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

    false
}

#[cfg(test)]
#[path = "tests/command_parse.rs"]
mod tests;
