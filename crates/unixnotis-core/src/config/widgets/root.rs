use serde::{Deserialize, Serialize};

use super::{
    CardWidgetConfig, SliderWidgetConfig, StatWidgetConfig, ToggleLayout, ToggleWidgetConfig,
};

/// Shared spacing profile for the built-in widget stack
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum WidgetDensity {
    /// Preserve roomy touch targets and section spacing
    #[default]
    Comfortable,
    /// Reduce non-interactive padding while keeping controls usable
    Compact,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct WidgetsConfig {
    // New installations serialize the current compact grid values from `Default`
    // Older files that omit a newer key use the field-level compatibility defaults below
    // This distinction keeps theme bundles visually stable across UnixNotis upgrades
    /// Shared density profile applied before theme-specific CSS
    #[serde(default = "legacy_widget_density")]
    pub density: WidgetDensity,
    pub volume: SliderWidgetConfig,
    pub brightness: SliderWidgetConfig,
    /// Controls whether toggle buttons expose GTK tooltips on hover
    pub toggle_tooltips: bool,
    /// Controls icon and label placement inside toggle cards
    pub toggle_layout: ToggleLayout,
    /// Fixed column count used by the toggle grid
    #[serde(default = "legacy_toggle_columns")]
    pub toggle_columns: usize,
    /// Fixed column count used by the stat grid
    #[serde(default = "legacy_stat_columns")]
    pub stat_columns: usize,
    /// Fixed column count used by the card grid
    #[serde(default = "legacy_card_columns")]
    pub card_columns: usize,
    pub toggles: Vec<ToggleWidgetConfig>,
    pub stats: Vec<StatWidgetConfig>,
    #[serde(default = "legacy_cards")]
    pub cards: Vec<CardWidgetConfig>,
    pub refresh_interval_ms: u64,
    pub refresh_interval_slow_ms: u64,
}

// These helpers must remain independent from `WidgetsConfig::default()`
// Pointing them at current defaults would recreate the same legacy-preset regression

// Configs written before density was configurable used the comfortable spacing profile
const fn legacy_widget_density() -> WidgetDensity {
    WidgetDensity::Comfortable
}

// Four toggle columns preserve the original single-row quick-action layout
const fn legacy_toggle_columns() -> usize {
    4
}

// Two columns preserve the original balanced stat grid
const fn legacy_stat_columns() -> usize {
    2
}

// Two columns preserve the original paired information-card layout
const fn legacy_card_columns() -> usize {
    2
}

// Calendar and weather were visible when an older partial widget table omitted cards
// Build the current card definitions first so command and metadata fixes still reach legacy configs
fn legacy_cards() -> Vec<CardWidgetConfig> {
    let mut cards = vec![
        CardWidgetConfig::default_calendar(),
        CardWidgetConfig::default_weather(),
    ];
    for card in &mut cards {
        card.enabled = true;
    }
    cards
}

impl Default for WidgetsConfig {
    fn default() -> Self {
        // This block is the first-install profile and may evolve with the shipped default theme
        // Legacy missing-field behavior belongs in the helpers above and must not follow these values
        Self {
            // Compact density keeps the complete stock control set visible on common displays
            density: WidgetDensity::Compact,
            volume: SliderWidgetConfig::default_volume(),
            brightness: SliderWidgetConfig::default_brightness(),
            // Tooltips stay opt-in so compact panels do not add hover-only noise
            toggle_tooltips: false,
            toggle_layout: ToggleLayout::Horizontal,
            // Two wide toggle columns keep labels readable in a narrow side panel
            toggle_columns: 2,
            // CPU, memory, and battery share one balanced status row
            stat_columns: 3,
            // Optional information cards receive the full width when enabled
            card_columns: 1,
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

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
