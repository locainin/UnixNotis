//! Detection helpers for existing Hyprland startup lines

use super::super::HYPR_IMPORT_VARS;
use super::paths::HyprlandConfigSyntax;

const HYPR_RESTART_COMMAND: &str = "systemctl --user --no-block restart unixnotis-daemon.service";

pub(in crate::actions::hyprland) fn hyprland_import_line(syntax: HyprlandConfigSyntax) -> String {
    // Keep the import list in one place so installer and Hyprland stay aligned
    hyprland_startup_line(
        syntax,
        &format!(
            "systemctl --user import-environment {}",
            HYPR_IMPORT_VARS.join(" ")
        ),
    )
}

pub(in crate::actions::hyprland) fn hyprland_dbus_update_line(
    syntax: HyprlandConfigSyntax,
) -> String {
    // Build the login-time D-Bus sync line from the same variable list
    hyprland_startup_line(syntax, &hyprland_dbus_update_command())
}

pub(in crate::actions::hyprland) fn hyprland_restart_line(syntax: HyprlandConfigSyntax) -> String {
    // Keep the restart command identical across formats so detection can stay conservative
    hyprland_startup_line(syntax, HYPR_RESTART_COMMAND)
}

pub(in crate::actions::hyprland) fn has_exec_once_dbus_update(contents: &str) -> bool {
    // Accept the old --systemd --all form too so upgrades do not duplicate lines
    has_startup_command(
        contents,
        "dbus-update-activation-environment --systemd --all",
    ) || has_startup_command(contents, &hyprland_dbus_update_command())
}

pub(in crate::actions::hyprland) fn has_exec_once_import(contents: &str, vars: &[&str]) -> bool {
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

pub(in crate::actions::hyprland) fn has_exec_once_restart(contents: &str) -> bool {
    // Match only an active unixnotis-daemon restart line, not comments or generic systemctl
    has_startup_command(contents, HYPR_RESTART_COMMAND)
        || contents.lines().any(|line| {
            let Some(trimmed) = active_startup_line(line) else {
                return false;
            };
            trimmed.contains("systemctl --user")
                && trimmed.contains("restart")
                && trimmed.contains("unixnotis-daemon.service")
        })
}

fn hyprland_startup_line(syntax: HyprlandConfigSyntax, command: &str) -> String {
    // Lua startup commands must run after Hyprland finishes loading the config
    match syntax {
        HyprlandConfigSyntax::Lua => format!("    hl.exec_cmd({})", lua_string(command)),
        HyprlandConfigSyntax::Hyprlang => format!("exec-once = {command}"),
    }
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

fn has_startup_command(contents: &str, needle: &str) -> bool {
    // Only count live startup lines so commented examples do not block updates
    contents.lines().any(|line| {
        let Some(trimmed) = active_startup_line(line) else {
            return false;
        };
        trimmed.contains(needle)
    })
}

fn hyprland_dbus_update_command() -> String {
    format!(
        "dbus-update-activation-environment {}",
        HYPR_IMPORT_VARS.join(" ")
    )
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
