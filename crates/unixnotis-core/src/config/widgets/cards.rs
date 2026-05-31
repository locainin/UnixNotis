use serde::{Deserialize, Serialize};

use super::WidgetPluginConfig;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct CardWidgetConfig {
    pub enabled: bool,
    pub kind: Option<String>,
    pub layout: CardLayout,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: Option<String>,
    pub cmd: Option<String>,
    /// External plugin source for this card (preferred over cmd when set)
    pub plugin: Option<WidgetPluginConfig>,
    pub min_height: i32,
    pub monospace: bool,
    /// Decorative dot count for banner/carousel-styled cards
    pub carousel_dots: usize,
    /// Show decorative previous/next controls on banner-style cards
    pub carousel_arrows: bool,
}

impl CardWidgetConfig {
    pub(super) fn default_calendar() -> Self {
        Self {
            enabled: true,
            kind: Some("calendar".to_string()),
            layout: CardLayout::Default,
            title: "Calendar".to_string(),
            subtitle: None,
            icon: Some("x-office-calendar-symbolic".to_string()),
            // No command means the center renders the built-in GTK calendar widget
            cmd: None,
            plugin: None,
            min_height: 180,
            monospace: false,
            carousel_dots: 0,
            carousel_arrows: false,
        }
    }

    pub(super) fn default_weather() -> Self {
        Self {
            enabled: true,
            kind: Some("weather".to_string()),
            layout: CardLayout::Default,
            title: "Weather".to_string(),
            subtitle: Some("No data".to_string()),
            icon: Some("weather-clear-symbolic".to_string()),
            // Weather is a styled placeholder until the user supplies a command or plugin
            cmd: None,
            plugin: None,
            min_height: 160,
            monospace: false,
            carousel_dots: 0,
            carousel_arrows: false,
        }
    }
}

impl Default for CardWidgetConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            kind: None,
            layout: CardLayout::Default,
            title: "Card".to_string(),
            subtitle: None,
            icon: None,
            cmd: None,
            plugin: None,
            min_height: 120,
            monospace: false,
            carousel_dots: 0,
            carousel_arrows: false,
        }
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CardLayout {
    #[default]
    Default,
    Banner,
    ImageRow,
}
