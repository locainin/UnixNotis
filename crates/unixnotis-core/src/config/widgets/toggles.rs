use serde::{Deserialize, Serialize};

use crate::config::commands;

/// Icon and label orientation for toggle cards
#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ToggleLayout {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ToggleWidgetConfig {
    pub enabled: bool,
    /// Stable identifier used for CSS classes and future migrations
    /// Labels can change without changing the toggle identity
    #[serde(alias = "id")]
    pub kind: Option<String>,
    pub label: String,
    pub icon: String,
    pub state_cmd: Option<String>,
    /// Optional command run for every user click before state is refreshed
    ///
    /// Useful for custom buttons that do not map cleanly to separate on/off commands
    pub toggle_cmd: Option<String>,
    pub on_cmd: Option<String>,
    pub off_cmd: Option<String>,
    pub watch_cmd: Option<String>,
}

impl ToggleWidgetConfig {
    pub(super) fn default_wifi() -> Self {
        Self {
            enabled: true,
            kind: Some(commands::TOGGLE_KIND_WIFI.to_string()),
            label: "Wi-Fi".to_string(),
            icon: "network-wireless-signal-excellent-symbolic".to_string(),
            state_cmd: Some(commands::WIFI_STATE_NMCLI.to_string()),
            toggle_cmd: None,
            on_cmd: Some(commands::WIFI_ON_NMCLI.to_string()),
            off_cmd: Some(commands::WIFI_OFF_NMCLI.to_string()),
            watch_cmd: Some(commands::WIFI_WATCH_NMCLI.to_string()),
        }
    }

    pub(super) fn default_bluetooth() -> Self {
        Self {
            enabled: true,
            kind: Some(commands::TOGGLE_KIND_BLUETOOTH.to_string()),
            label: "Bluetooth".to_string(),
            icon: "bluetooth-active-symbolic".to_string(),
            state_cmd: Some(commands::BLUETOOTH_STATE_BLUETOOTHCTL.to_string()),
            toggle_cmd: None,
            on_cmd: Some(commands::BLUETOOTH_ON_BLUETOOTHCTL.to_string()),
            off_cmd: Some(commands::BLUETOOTH_OFF_BLUETOOTHCTL.to_string()),
            // D-Bus monitoring avoids TTY requirements and follows BlueZ state changes
            watch_cmd: Some(commands::BLUETOOTH_WATCH_DBUS.to_string()),
        }
    }

    pub(super) fn default_airplane() -> Self {
        Self {
            enabled: true,
            kind: Some(commands::TOGGLE_KIND_AIRPLANE.to_string()),
            label: "Airplane".to_string(),
            icon: "airplane-mode-symbolic".to_string(),
            // Airplane reads active only when every rfkill device is soft-blocked
            state_cmd: Some(commands::AIRPLANE_STATE_CMD.to_string()),
            toggle_cmd: None,
            on_cmd: Some(commands::AIRPLANE_ON_CMD.to_string()),
            off_cmd: Some(commands::AIRPLANE_OFF_CMD.to_string()),
            watch_cmd: Some(commands::AIRPLANE_WATCH_CMD.to_string()),
        }
    }

    pub(super) fn default_night() -> Self {
        Self {
            enabled: true,
            kind: Some(commands::TOGGLE_KIND_NIGHT.to_string()),
            label: "Night".to_string(),
            icon: "weather-clear-night-symbolic".to_string(),
            // Shipped scripts keep backend fallback logic in editable files
            state_cmd: Some("scripts/unixnotis-blue-light-state".to_string()),
            toggle_cmd: None,
            on_cmd: Some("scripts/unixnotis-blue-light-on".to_string()),
            off_cmd: Some("scripts/unixnotis-blue-light-off".to_string()),
            watch_cmd: None,
        }
    }
}

impl Default for ToggleWidgetConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            kind: None,
            label: "Toggle".to_string(),
            icon: "applications-system-symbolic".to_string(),
            state_cmd: None,
            toggle_cmd: None,
            on_cmd: None,
            off_cmd: None,
            watch_cmd: None,
        }
    }
}
