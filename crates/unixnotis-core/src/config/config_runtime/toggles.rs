//! Runtime adjustments for toggle widget backends.

use std::env;

use super::super::config_commands::*;
use super::super::ToggleWidgetConfig;
use crate::program_in_path;

// Toggle command templates used when normalizing runtime defaults.
const LEGACY_AIRPLANE_STATE_CMD: &str = "rfkill list all | grep -q \"Soft blocked: yes\"";

pub(in super::super) fn apply_toggle_backends(toggles: &mut [ToggleWidgetConfig]) {
    let gammastep_available = program_in_path("gammastep");
    let wlsunset_available = program_in_path("wlsunset");
    let bluetoothctl_available = program_in_path("bluetoothctl");
    let dbus_monitor_available = program_in_path("dbus-monitor");
    let rfkill_available = program_in_path("rfkill");
    let systemctl_available = program_in_path("systemctl");
    let hyprsunset_available = program_in_path("hyprsunset");
    let hyprctl_available = program_in_path("hyprctl");
    // Hyprsunset should only be selected when a Hyprland compositor is detected.
    let hyprland_session = is_hyprland_session();

    for toggle in toggles {
        // Keep default toggles aligned with the latest backend expectations.
        if is_bluetooth_toggle(toggle) {
            apply_bluetooth_defaults(
                toggle,
                bluetoothctl_available,
                dbus_monitor_available,
                rfkill_available,
                systemctl_available,
            );
        }
        if is_airplane_toggle(toggle) {
            apply_airplane_defaults(toggle);
        }
        if is_night_toggle(toggle) {
            apply_night_defaults(
                toggle,
                gammastep_available,
                wlsunset_available,
                hyprsunset_available,
                hyprctl_available,
                hyprland_session,
            );
        }
    }
}

fn is_airplane_toggle(toggle: &ToggleWidgetConfig) -> bool {
    toggle_kind_eq(toggle, TOGGLE_KIND_AIRPLANE)
}

fn is_bluetooth_toggle(toggle: &ToggleWidgetConfig) -> bool {
    toggle_kind_eq(toggle, TOGGLE_KIND_BLUETOOTH)
}

fn is_night_toggle(toggle: &ToggleWidgetConfig) -> bool {
    toggle_kind_eq(toggle, TOGGLE_KIND_NIGHT)
}

fn toggle_kind_eq(toggle: &ToggleWidgetConfig, kind: &str) -> bool {
    toggle
        .kind
        .as_deref()
        .map(|value| value.trim().eq_ignore_ascii_case(kind))
        .unwrap_or_else(|| {
            // Preserve older configs by using label/command inference when kind is missing.
            if toggle.label.trim().eq_ignore_ascii_case(kind) {
                return true;
            }
            matches_default_kind(toggle, kind)
        })
}

fn matches_default_kind(toggle: &ToggleWidgetConfig, kind: &str) -> bool {
    match kind {
        TOGGLE_KIND_BLUETOOTH => {
            let state = toggle.state_cmd.as_deref().unwrap_or_default();
            let on_cmd = toggle.on_cmd.as_deref().unwrap_or_default();
            let off_cmd = toggle.off_cmd.as_deref().unwrap_or_default();
            let watch_cmd = toggle.watch_cmd.as_deref().unwrap_or_default();
            state == BLUETOOTH_STATE_BLUETOOTHCTL
                || state == BLUETOOTH_STATE_RFKILL
                || state == BLUETOOTH_STATE_SYSTEMCTL
                || on_cmd == BLUETOOTH_ON_BLUETOOTHCTL
                || on_cmd == BLUETOOTH_ON_RFKILL
                || on_cmd == BLUETOOTH_ON_SYSTEMCTL
                || off_cmd == BLUETOOTH_OFF_BLUETOOTHCTL
                || off_cmd == BLUETOOTH_OFF_RFKILL
                || off_cmd == BLUETOOTH_OFF_SYSTEMCTL
                || watch_cmd == BLUETOOTH_WATCH_DBUS
                || watch_cmd == BLUETOOTH_WATCH_RFKILL
        }
        TOGGLE_KIND_AIRPLANE => {
            toggle
                .state_cmd
                .as_deref()
                // Match default and legacy state commands for older configs.
                .map(|state| state == AIRPLANE_STATE_CMD || is_legacy_airplane_state(state))
                .unwrap_or(false)
                || toggle
                    .on_cmd
                    .as_deref()
                    .map(|cmd| cmd == AIRPLANE_ON_CMD)
                    .unwrap_or(false)
                || toggle
                    .off_cmd
                    .as_deref()
                    .map(|cmd| cmd == AIRPLANE_OFF_CMD)
                    .unwrap_or(false)
                || toggle
                    .watch_cmd
                    .as_deref()
                    .map(|cmd| cmd == AIRPLANE_WATCH_CMD)
                    .unwrap_or(false)
        }
        TOGGLE_KIND_NIGHT => is_default_night_backend(toggle),
        _ => false,
    }
}

fn is_hyprland_session() -> bool {
    // Detect Hyprland via env markers so hyprsunset is only selected on compatible sessions.
    if env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        return true;
    }
    let desktop = env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let session = env::var("XDG_SESSION_DESKTOP")
        .unwrap_or_default()
        .to_ascii_lowercase();
    desktop.contains("hyprland") || session.contains("hyprland")
}

fn apply_bluetooth_defaults(
    toggle: &mut ToggleWidgetConfig,
    bluetoothctl_available: bool,
    dbus_monitor_available: bool,
    rfkill_available: bool,
    systemctl_available: bool,
) {
    let uses_bluetoothctl = toggle_uses_backend(toggle, "bluetoothctl");
    let uses_rfkill = toggle_uses_backend(toggle, "rfkill");
    let uses_systemctl = toggle_uses_backend(toggle, "systemctl");

    // Prefer bluetoothctl for accurate power state when available.
    if bluetoothctl_available {
        // bluetoothctl reports adapter Powered/PowerState, which matches UI semantics.
        if is_blank(&toggle.state_cmd) || uses_rfkill || uses_systemctl {
            toggle.state_cmd = Some(BLUETOOTH_STATE_BLUETOOTHCTL.to_string());
        }
        if is_blank(&toggle.on_cmd) || uses_rfkill || uses_systemctl {
            toggle.on_cmd = Some(BLUETOOTH_ON_BLUETOOTHCTL.to_string());
        }
        if is_blank(&toggle.off_cmd) || uses_rfkill || uses_systemctl {
            toggle.off_cmd = Some(BLUETOOTH_OFF_BLUETOOTHCTL.to_string());
        }
    } else if rfkill_available {
        // rfkill mirrors adapter soft-block state when bluetoothctl is unavailable.
        if is_blank(&toggle.state_cmd) || uses_bluetoothctl || uses_systemctl {
            toggle.state_cmd = Some(BLUETOOTH_STATE_RFKILL.to_string());
        }
        if is_blank(&toggle.on_cmd) || uses_bluetoothctl || uses_systemctl {
            toggle.on_cmd = Some(BLUETOOTH_ON_RFKILL.to_string());
        }
        if is_blank(&toggle.off_cmd) || uses_bluetoothctl || uses_systemctl {
            toggle.off_cmd = Some(BLUETOOTH_OFF_RFKILL.to_string());
        }
    } else if systemctl_available {
        // systemctl is a last-resort fallback when Bluetooth tooling is missing.
        if is_blank(&toggle.state_cmd) || uses_bluetoothctl || uses_rfkill {
            toggle.state_cmd = Some(BLUETOOTH_STATE_SYSTEMCTL.to_string());
        }
        if is_blank(&toggle.on_cmd) || uses_bluetoothctl || uses_rfkill {
            toggle.on_cmd = Some(BLUETOOTH_ON_SYSTEMCTL.to_string());
        }
        if is_blank(&toggle.off_cmd) || uses_bluetoothctl || uses_rfkill {
            toggle.off_cmd = Some(BLUETOOTH_OFF_SYSTEMCTL.to_string());
        }
    }

    // D-Bus monitoring is lightweight and does not require a controlling TTY.
    let watch_cmd = toggle.watch_cmd.as_deref().unwrap_or_default();
    let watch_uses_bluetoothctl = watch_cmd.contains("bluetoothctl");
    let watch_uses_dbus = watch_cmd.contains("dbus-monitor");
    let watch_uses_rfkill = watch_cmd.contains("rfkill");
    // Drop watch commands that reference unavailable backends to avoid stale UI state.
    let watch_missing = (watch_uses_bluetoothctl && !bluetoothctl_available)
        || (watch_uses_dbus && !dbus_monitor_available)
        || (watch_uses_rfkill && !rfkill_available);
    // Treat legacy or blank watchers as candidates for replacement.
    let watch_default = is_blank(&toggle.watch_cmd)
        || watch_uses_bluetoothctl
        || watch_uses_dbus
        || watch_uses_rfkill;

    if watch_default || watch_missing {
        // Prefer D-Bus monitoring; bluetoothctl requires a TTY and often fails without one.
        // rfkill provides a lightweight fallback when D-Bus monitoring is unavailable.
        let desired = if dbus_monitor_available {
            Some(BLUETOOTH_WATCH_DBUS)
        } else if rfkill_available {
            Some(BLUETOOTH_WATCH_RFKILL)
        } else {
            None
        };
        toggle.watch_cmd = desired.map(|cmd| cmd.to_string());
    }
}

fn apply_airplane_defaults(toggle: &mut ToggleWidgetConfig) {
    // Refresh legacy configs that treated any soft block as airplane mode.
    let state_missing = is_blank(&toggle.state_cmd);
    let state_legacy = toggle
        .state_cmd
        .as_deref()
        .map(is_legacy_airplane_state)
        .unwrap_or(false);
    if state_missing || state_legacy {
        toggle.state_cmd = Some(AIRPLANE_STATE_CMD.to_string());
    }
    if is_blank(&toggle.on_cmd) {
        toggle.on_cmd = Some(AIRPLANE_ON_CMD.to_string());
    }
    if is_blank(&toggle.off_cmd) {
        toggle.off_cmd = Some(AIRPLANE_OFF_CMD.to_string());
    }
    if is_blank(&toggle.watch_cmd) {
        toggle.watch_cmd = Some(AIRPLANE_WATCH_CMD.to_string());
    }
}

fn apply_night_defaults(
    toggle: &mut ToggleWidgetConfig,
    gammastep_available: bool,
    wlsunset_available: bool,
    hyprsunset_available: bool,
    hyprctl_available: bool,
    hyprland_session: bool,
) {
    let uses_gammastep = toggle_uses_backend(toggle, "gammastep");
    let uses_wlsunset = toggle_uses_backend(toggle, "wlsunset");
    let uses_hyprsunset = toggle_uses_backend(toggle, "hyprsunset");
    let hyprsunset_preferred = hyprsunset_available && hyprctl_available && hyprland_session;
    // Switch default backends to hyprsunset when running on Hyprland.
    let prefers_hyprsunset =
        hyprsunset_preferred && is_default_night_backend(toggle) && !uses_hyprsunset;
    // Switch when a referenced backend is missing but the fallback exists.
    // This prevents configs from pointing at missing executables after upgrades.
    let default_backend = is_default_night_backend(toggle);
    let backend_unavailable = default_backend
        && ((uses_hyprsunset
            && !hyprsunset_preferred
            && (gammastep_available || wlsunset_available))
            || (uses_gammastep
                && !gammastep_available
                && (hyprsunset_preferred || wlsunset_available))
            || (uses_wlsunset
                && !wlsunset_available
                && (hyprsunset_preferred || gammastep_available)));
    // Only rewrite when commands are blank, legacy, or mismatched with available backends.
    let needs_update = is_blank(&toggle.state_cmd)
        || is_blank(&toggle.on_cmd)
        || is_blank(&toggle.off_cmd)
        || is_legacy_night_toggle(toggle)
        || prefers_hyprsunset
        || backend_unavailable;
    if !needs_update {
        return;
    }

    // Prefer hyprsunset on Hyprland because it uses compositor-native CTM control.
    if hyprsunset_preferred {
        // Keep the toggle semantics aligned with a fixed "night" temperature band.
        toggle.state_cmd = Some(NIGHT_HYPRSUNSET_STATE.to_string());
        toggle.on_cmd = Some(NIGHT_HYPRSUNSET_ON.to_string());
        toggle.off_cmd = Some(NIGHT_HYPRSUNSET_OFF.to_string());
        toggle.watch_cmd = None;
        return;
    }

    // Prefer gammastep with a fixed temperature to avoid geoclue dependency.
    // Using identical day/night values keeps the process running for state checks.
    if gammastep_available {
        toggle.state_cmd = Some(NIGHT_GAMMASTEP_STATE.to_string());
        toggle.on_cmd = Some(NIGHT_GAMMASTEP_ON.to_string());
        toggle.off_cmd = Some(NIGHT_GAMMASTEP_OFF.to_string());
        toggle.watch_cmd = None;
        return;
    }

    // Fall back to wlsunset when gammastep is unavailable.
    if wlsunset_available {
        toggle.state_cmd = Some(NIGHT_WLSUNSET_STATE.to_string());
        toggle.on_cmd = Some(NIGHT_WLSUNSET_ON.to_string());
        toggle.off_cmd = Some(NIGHT_WLSUNSET_OFF.to_string());
        toggle.watch_cmd = None;
    }
}

fn is_blank(value: &Option<String>) -> bool {
    value
        .as_deref()
        .map(|cmd| cmd.trim().is_empty())
        .unwrap_or(true)
}

fn is_legacy_airplane_state(cmd: &str) -> bool {
    cmd.trim() == LEGACY_AIRPLANE_STATE_CMD
}

fn is_legacy_night_toggle(toggle: &ToggleWidgetConfig) -> bool {
    let on_cmd = toggle.on_cmd.as_deref().unwrap_or_default();
    let off_cmd = toggle.off_cmd.as_deref().unwrap_or_default();
    let state_cmd = toggle.state_cmd.as_deref().unwrap_or_default();
    // Legacy configs used combined state checks and one-shot commands.
    let legacy_state = state_cmd.contains("gammastep")
        && state_cmd.contains("wlsunset")
        && state_cmd.contains("||");
    let legacy_on = on_cmd.contains("command -v gammastep")
        || on_cmd.contains("gammastep -O")
        || on_cmd.contains("wlsunset -T");
    let legacy_off =
        off_cmd.contains("pkill -x gammastep") && off_cmd.contains("pkill -x wlsunset");
    legacy_state || legacy_on || legacy_off
}

fn toggle_uses_backend(toggle: &ToggleWidgetConfig, backend: &str) -> bool {
    // Check all command slots so custom layouts still resolve the backend.
    let state_cmd = toggle.state_cmd.as_deref().unwrap_or_default();
    let on_cmd = toggle.on_cmd.as_deref().unwrap_or_default();
    let off_cmd = toggle.off_cmd.as_deref().unwrap_or_default();
    state_cmd.contains(backend) || on_cmd.contains(backend) || off_cmd.contains(backend)
}

fn is_default_night_backend(toggle: &ToggleWidgetConfig) -> bool {
    // Detect stock night backends so Hyprland sessions can switch to hyprsunset safely.
    let state_cmd = toggle.state_cmd.as_deref().unwrap_or_default();
    let on_cmd = toggle.on_cmd.as_deref().unwrap_or_default();
    let off_cmd = toggle.off_cmd.as_deref().unwrap_or_default();
    let gammastep_default = state_cmd == NIGHT_GAMMASTEP_STATE
        && on_cmd == NIGHT_GAMMASTEP_ON
        && off_cmd == NIGHT_GAMMASTEP_OFF;
    let wlsunset_default = state_cmd == NIGHT_WLSUNSET_STATE
        && on_cmd == NIGHT_WLSUNSET_ON
        && off_cmd == NIGHT_WLSUNSET_OFF;
    let hyprsunset_default = state_cmd == NIGHT_HYPRSUNSET_STATE
        && on_cmd == NIGHT_HYPRSUNSET_ON
        && off_cmd == NIGHT_HYPRSUNSET_OFF;
    gammastep_default || wlsunset_default || hyprsunset_default
}
