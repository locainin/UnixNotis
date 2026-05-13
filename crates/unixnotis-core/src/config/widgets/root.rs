use serde::{Deserialize, Serialize};

use super::{
    CardWidgetConfig, SliderWidgetConfig, StatWidgetConfig, ToggleLayout, ToggleWidgetConfig,
};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct WidgetsConfig {
    pub volume: SliderWidgetConfig,
    pub brightness: SliderWidgetConfig,
    /// Controls whether toggle buttons expose GTK tooltips on hover
    pub toggle_tooltips: bool,
    /// Controls icon and label placement inside toggle cards
    pub toggle_layout: ToggleLayout,
    /// Fixed column count used by the toggle grid
    pub toggle_columns: usize,
    /// Fixed column count used by the stat grid
    pub stat_columns: usize,
    /// Fixed column count used by the card grid
    pub card_columns: usize,
    pub toggles: Vec<ToggleWidgetConfig>,
    pub stats: Vec<StatWidgetConfig>,
    pub cards: Vec<CardWidgetConfig>,
    pub refresh_interval_ms: u64,
    pub refresh_interval_slow_ms: u64,
}

impl Default for WidgetsConfig {
    fn default() -> Self {
        Self {
            volume: SliderWidgetConfig::default_volume(),
            brightness: SliderWidgetConfig::default_brightness(),
            // Tooltips stay opt-in so compact panels do not add hover-only noise
            toggle_tooltips: false,
            toggle_layout: ToggleLayout::Horizontal,
            toggle_columns: 4,
            stat_columns: 2,
            card_columns: 2,
            toggles: vec![
                ToggleWidgetConfig::default_wifi(),
                ToggleWidgetConfig::default_bluetooth(),
                ToggleWidgetConfig::default_airplane(),
                ToggleWidgetConfig::default_night(),
            ],
            stats: vec![
                StatWidgetConfig::default_cpu(),
                StatWidgetConfig::default_memory(),
                StatWidgetConfig::default_battery(),
            ],
            cards: vec![
                CardWidgetConfig::default_calendar(),
                CardWidgetConfig::default_weather(),
            ],
            refresh_interval_ms: 1000,
            refresh_interval_slow_ms: 3000,
        }
    }
}
