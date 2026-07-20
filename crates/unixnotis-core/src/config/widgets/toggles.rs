use serde::{Deserialize, Serialize};

use crate::config::command::defaults as commands;
use crate::CommandSpec;

/// Icon and label orientation for toggle cards
#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ToggleLayout {
    #[default]
    Horizontal,
    Vertical,
}

/// Built-in state parser used after a direct toggle command completes
#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToggleBackend {
    Rfkill,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct ToggleWidgetConfig {
    pub enabled: bool,
    /// Stable identifier used for CSS classes and future migrations
    /// Labels can change without changing the toggle identity
    #[serde(alias = "id")]
    pub kind: Option<String>,
    pub label: String,
    pub icon: String,
    pub icon_asset: Option<String>,
    pub backend: Option<ToggleBackend>,
    pub state_cmd: Option<CommandSpec>,
    /// Optional command run for every user click before state is refreshed
    ///
    /// Useful for custom buttons that do not map cleanly to separate on/off commands
    pub toggle_cmd: Option<CommandSpec>,
    pub on_cmd: Option<CommandSpec>,
    pub off_cmd: Option<CommandSpec>,
    pub watch_cmd: Option<CommandSpec>,
}

impl ToggleWidgetConfig {
    pub(super) fn default_wifi() -> Self {
        Self {
            enabled: true,
            kind: Some(commands::TOGGLE_KIND_WIFI.to_string()),
            label: "Wi-Fi".to_string(),
            icon: "network-wireless-signal-excellent-symbolic".to_string(),
            icon_asset: None,
            backend: None,
            state_cmd: Some(commands::wifi_state()),
            toggle_cmd: None,
            on_cmd: Some(commands::wifi_on()),
            off_cmd: Some(commands::wifi_off()),
            watch_cmd: Some(commands::wifi_watch()),
        }
    }

    pub(super) fn default_bluetooth() -> Self {
        Self {
            enabled: true,
            kind: Some(commands::TOGGLE_KIND_BLUETOOTH.to_string()),
            label: "Bluetooth".to_string(),
            icon: "bluetooth-active-symbolic".to_string(),
            icon_asset: None,
            backend: None,
            state_cmd: Some(commands::bluetooth_state()),
            toggle_cmd: None,
            on_cmd: Some(commands::bluetooth_on()),
            off_cmd: Some(commands::bluetooth_off()),
            // D-Bus monitoring avoids TTY requirements and follows BlueZ state changes
            watch_cmd: Some(commands::bluetooth_watch()),
        }
    }

    pub(super) fn default_airplane() -> Self {
        Self {
            enabled: true,
            kind: Some(commands::TOGGLE_KIND_AIRPLANE.to_string()),
            label: "Airplane".to_string(),
            icon: "airplane-mode-symbolic".to_string(),
            icon_asset: None,
            backend: Some(ToggleBackend::Rfkill),
            // Airplane state is parsed from rfkill's machine-readable JSON output
            state_cmd: Some(commands::airplane_state()),
            toggle_cmd: None,
            on_cmd: Some(commands::airplane_on()),
            off_cmd: Some(commands::airplane_off()),
            watch_cmd: Some(commands::airplane_watch()),
        }
    }

    pub(super) fn default_night() -> Self {
        Self {
            enabled: true,
            kind: Some(commands::TOGGLE_KIND_NIGHT.to_string()),
            label: "Night".to_string(),
            icon: "weather-clear-night-symbolic".to_string(),
            icon_asset: None,
            backend: None,
            // Shipped scripts keep backend fallback logic in editable files
            state_cmd: Some(CommandSpec::direct(
                "scripts/unixnotis-blue-light-state",
                std::iter::empty::<&str>(),
            )),
            toggle_cmd: None,
            on_cmd: Some(CommandSpec::direct(
                "scripts/unixnotis-blue-light-on",
                std::iter::empty::<&str>(),
            )),
            off_cmd: Some(CommandSpec::direct(
                "scripts/unixnotis-blue-light-off",
                std::iter::empty::<&str>(),
            )),
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
            icon_asset: None,
            backend: None,
            state_cmd: None,
            toggle_cmd: None,
            on_cmd: None,
            off_cmd: None,
            watch_cmd: None,
        }
    }
}
