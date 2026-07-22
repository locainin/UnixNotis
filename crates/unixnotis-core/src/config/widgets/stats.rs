use serde::{Deserialize, Serialize};

use super::WidgetPluginConfig;
use crate::CommandSpec;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct StatWidgetConfig {
    pub enabled: bool,
    pub label: String,
    pub icon: Option<String>,
    pub icon_asset: Option<String>,
    pub kind: Option<String>,
    pub cmd: Option<CommandSpec>,
    /// External plugin source for this stat (preferred over cmd when set)
    pub plugin: Option<WidgetPluginConfig>,
    pub min_height: i32,
}

impl StatWidgetConfig {
    pub(super) fn default_cpu() -> Self {
        Self {
            enabled: true,
            label: "CPU".to_string(),
            icon: Some("utilities-system-monitor-symbolic".to_string()),
            icon_asset: None,
            kind: Some("cpu".to_string()),
            // Builtins avoid shelling out for common fast-refresh stats
            cmd: Some(CommandSpec::direct(
                "builtin:cpu",
                std::iter::empty::<&str>(),
            )),
            plugin: None,
            min_height: 72,
        }
    }

    pub(super) fn default_memory() -> Self {
        Self {
            enabled: true,
            label: "RAM".to_string(),
            icon: Some("drive-harddisk-symbolic".to_string()),
            icon_asset: None,
            kind: Some("ram".to_string()),
            // Memory comes from the same builtin path so defaults stay cheap to poll
            cmd: Some(CommandSpec::direct(
                "builtin:memory",
                std::iter::empty::<&str>(),
            )),
            plugin: None,
            min_height: 72,
        }
    }

    pub(super) fn default_battery() -> Self {
        Self {
            enabled: true,
            label: "Battery".to_string(),
            icon: Some("battery-full-symbolic".to_string()),
            icon_asset: None,
            kind: Some("battery".to_string()),
            // Battery remains optional at runtime; systems without a battery render fallback text
            cmd: Some(CommandSpec::direct(
                "builtin:battery",
                std::iter::empty::<&str>(),
            )),
            plugin: None,
            min_height: 72,
        }
    }
}

impl Default for StatWidgetConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            label: "Stat".to_string(),
            icon: None,
            icon_asset: None,
            kind: None,
            cmd: None,
            plugin: None,
            min_height: 72,
        }
    }
}
