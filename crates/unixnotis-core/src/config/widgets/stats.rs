use serde::{Deserialize, Serialize};

use super::WidgetPluginConfig;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct StatWidgetConfig {
    pub enabled: bool,
    pub label: String,
    pub icon: Option<String>,
    pub kind: Option<String>,
    pub cmd: Option<String>,
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
            kind: None,
            cmd: Some("builtin:cpu".to_string()),
            plugin: None,
            min_height: 72,
        }
    }

    pub(super) fn default_memory() -> Self {
        Self {
            enabled: true,
            label: "RAM".to_string(),
            icon: Some("drive-harddisk-symbolic".to_string()),
            kind: None,
            cmd: Some("builtin:memory".to_string()),
            plugin: None,
            min_height: 72,
        }
    }

    pub(super) fn default_battery() -> Self {
        Self {
            enabled: true,
            label: "Battery".to_string(),
            icon: Some("battery-full-symbolic".to_string()),
            kind: None,
            cmd: Some("builtin:battery".to_string()),
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
            kind: None,
            cmd: None,
            plugin: None,
            min_height: 72,
        }
    }
}
