//! Panel action layout and label/icon configuration

use serde::{Deserialize, Serialize};

/// Location for the panel-wide clear action
#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PanelClearButtonPlacement {
    #[default]
    ActionRow,
    NotificationHeader,
    Hidden,
}

/// Known panel action roles that can be reordered by config
#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum PanelActionId {
    Widgets,
    Dnd,
    Clear,
    Search,
}

/// Return the stock left-to-right panel action order
pub fn default_panel_action_order() -> Vec<PanelActionId> {
    vec![
        PanelActionId::Widgets,
        PanelActionId::Dnd,
        PanelActionId::Clear,
        PanelActionId::Search,
    ]
}

/// Configurable label/icon data for built-in panel actions
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct PanelActionConfig {
    /// Text shown next to the icon when `icon_only` is false
    pub label: String,
    /// Icon name resolved through the active GTK icon theme
    pub icon: String,
    /// Tooltip shown on hover
    pub tooltip: String,
    /// Render this action as an icon-only control
    pub icon_only: bool,
}

impl PanelActionConfig {
    pub fn widgets() -> Self {
        Self {
            label: "Widgets".to_string(),
            icon: "applications-system-symbolic".to_string(),
            tooltip: "Toggle widget visibility".to_string(),
            icon_only: false,
        }
    }

    pub fn dnd() -> Self {
        Self {
            label: "DND".to_string(),
            icon: "weather-clear-night-symbolic".to_string(),
            tooltip: "Silence incoming notifications".to_string(),
            icon_only: false,
        }
    }

    pub fn clear() -> Self {
        Self {
            label: "Clear".to_string(),
            icon: "user-trash-symbolic".to_string(),
            tooltip: "Clear all notifications".to_string(),
            icon_only: false,
        }
    }

    pub fn search() -> Self {
        Self {
            label: "Search".to_string(),
            icon: "system-search-symbolic".to_string(),
            tooltip: "Toggle search".to_string(),
            icon_only: true,
        }
    }

    pub fn close() -> Self {
        Self {
            label: "Close".to_string(),
            icon: "window-close-symbolic".to_string(),
            tooltip: "Close panel".to_string(),
            icon_only: true,
        }
    }
}
