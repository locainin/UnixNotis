//! Detection helpers for existing Hyprland exec-once lines

use super::super::actions_env::HYPR_IMPORT_VARS;

pub(in crate::actions::hyprland) const HYPR_RESTART_CMD: &str =
    "exec-once = systemctl --user --no-block restart unixnotis-daemon.service";

pub(in crate::actions::hyprland) fn hyprland_import_line() -> String {
    // Keep the import list in one place so installer and Hyprland stay aligned
    format!(
        "exec-once = systemctl --user import-environment {}",
        HYPR_IMPORT_VARS.join(" ")
    )
}

pub(in crate::actions::hyprland) fn hyprland_dbus_update_line() -> String {
    // Build the login-time D-Bus sync line from the same variable list
    format!("exec-once = {}", hyprland_dbus_update_command())
}

pub(in crate::actions::hyprland) fn has_exec_once_dbus_update(contents: &str) -> bool {
    // Accept the old --systemd --all form too so upgrades do not duplicate lines
    has_exec_once_command(
        contents,
        "dbus-update-activation-environment --systemd --all",
    ) || has_exec_once_command(contents, &hyprland_dbus_update_command())
}

pub(in crate::actions::hyprland) fn has_exec_once_import(contents: &str, vars: &[&str]) -> bool {
    // Require every expected variable so partial imports do not count as complete
    contents.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return false;
        }
        if !(trimmed.starts_with("exec-once") && trimmed.contains("import-environment")) {
            return false;
        }
        vars.iter().all(|var| trimmed.contains(var))
    })
}

pub(in crate::actions::hyprland) fn has_exec_once_restart(contents: &str) -> bool {
    // Match only an active unixnotis-daemon restart line, not comments or generic systemctl
    contents.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return false;
        }
        trimmed.starts_with("exec-once")
            && trimmed.contains("systemctl --user")
            && trimmed.contains("restart")
            && trimmed.contains("unixnotis-daemon.service")
    })
}

fn has_exec_once_command(contents: &str, needle: &str) -> bool {
    // Only count live exec-once lines so commented examples do not block updates
    contents.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return false;
        }
        trimmed.starts_with("exec-once") && trimmed.contains(needle)
    })
}

fn hyprland_dbus_update_command() -> String {
    format!(
        "dbus-update-activation-environment {}",
        HYPR_IMPORT_VARS.join(" ")
    )
}
