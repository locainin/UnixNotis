//! Detection helpers for existing Hyprland startup lines

use super::paths::HyprlandConfigSyntax;

pub(in crate::actions::hyprland) fn hyprland_startup_line(
    syntax: HyprlandConfigSyntax,
    command: &str,
) -> String {
    // Lua startup commands must run after Hyprland finishes loading the config
    match syntax {
        HyprlandConfigSyntax::Lua => format!("    hl.exec_cmd({})", lua_string(command)),
        HyprlandConfigSyntax::Hyprlang => format!("exec-once = {command}"),
    }
}

pub(in crate::actions::hyprland) fn has_startup_command(contents: &str, needle: &str) -> bool {
    // Only count live startup lines so commented examples do not block updates
    contents.lines().any(|line| {
        let Some(trimmed) = active_startup_line(line) else {
            return false;
        };
        trimmed.contains(needle)
    })
}

pub(in crate::actions::hyprland) fn has_legacy_dbus_update(contents: &str) -> bool {
    // Accept the old --systemd --all form too so upgrades do not duplicate lines
    has_startup_command(
        contents,
        "dbus-update-activation-environment --systemd --all",
    )
}

pub(in crate::actions::hyprland) fn has_import_command_with_vars(
    contents: &str,
    vars: &[&str],
) -> bool {
    // Require every expected variable so partial imports do not count as complete
    contents.lines().any(|line| {
        let Some(trimmed) = active_startup_line(line) else {
            return false;
        };
        if !trimmed.contains("import-environment") {
            return false;
        }
        vars.iter().all(|var| trimmed.contains(var))
    })
}

fn active_startup_line(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    // Ignore both hyprlang and Lua comments so examples never count as active config
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("--") {
        return None;
    }
    if trimmed.starts_with("exec-once") || trimmed.starts_with("hl.exec_cmd") {
        return Some(trimmed);
    }
    None
}

fn lua_string(value: &str) -> String {
    // Commands are static today, but escaping keeps this helper safe if inputs grow later
    let mut quoted = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            _ => quoted.push(character),
        }
    }
    quoted.push('"');
    quoted
}
