//! Configuration types and defaults for UnixNotis.
//!
//! Keeps high-level config categories together and delegates detailed schemas
//! to focused modules for maintainability.

use serde::{Deserialize, Serialize};

use super::layout::{PanelConfig, PopupConfig};
use super::media::MediaConfig;
use super::rules::RuleConfig;
use super::theme::ThemeConfig;
use super::widget_config::WidgetsConfig;

/// Top-level configuration loaded from config.toml.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    // Main config sections
    pub general: GeneralConfig,
    pub inhibit: InhibitConfig,
    pub popups: PopupConfig,
    pub panel: PanelConfig,
    pub history: HistoryConfig,
    pub media: MediaConfig,
    pub widgets: WidgetsConfig,
    pub sound: SoundConfig,
    pub theme: ThemeConfig,
    pub rules: Vec<RuleConfig>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub dnd_default: bool,
    pub log_level: Option<String>,
}

/// Inhibit behavior controls how the daemon handles suppression requests.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct InhibitConfig {
    pub mode: InhibitMode,
}

impl Default for InhibitConfig {
    fn default() -> Self {
        Self {
            mode: InhibitMode::NoPopups,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InhibitMode {
    /// Store notifications but suppress popup rendering.
    NoPopups,
    /// Drop incoming notifications entirely while inhibited.
    DropAll,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct HistoryConfig {
    // Saved items
    pub max_entries: usize,
    // Live items
    pub max_active: usize,
    // Save transient items too
    pub transient_to_history: bool,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            max_entries: 200,
            // Match the daemon cap
            max_active: 12,
            transient_to_history: false,
        }
    }
}

/// Notification sound behavior.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SoundConfig {
    /// Enables sound playback when the daemon receives notifications.
    pub enabled: bool,
    /// Default named sound from the freedesktop sound theme.
    pub default_name: Option<String>,
    /// Default sound file path, resolves relative to the UnixNotis config dir.
    pub default_file: Option<String>,
    /// Directory containing custom sound files, resolves relative to config dir.
    pub default_dir: Option<String>,
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_name: Some("message-new-instant".to_string()),
            default_file: None,
            default_dir: None,
        }
    }
}
