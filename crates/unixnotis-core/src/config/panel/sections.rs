//! Panel section ordering configuration

use serde::{Deserialize, Serialize};

/// Top-level panel body sections that can be ordered by config
#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum PanelSection {
    Widgets,
    Notifications,
}

/// Return the stock top-to-bottom panel body section order
pub fn default_panel_section_order() -> Vec<PanelSection> {
    vec![PanelSection::Widgets, PanelSection::Notifications]
}

/// Top-level panel widget sections that can be ordered by config
#[derive(Debug, Copy, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum PanelWidgetSection {
    Media,
    Toggles,
    Sliders,
    Stats,
    Cards,
}

/// Return the stock top-to-bottom panel section order
pub fn default_panel_widget_order() -> Vec<PanelWidgetSection> {
    vec![
        PanelWidgetSection::Sliders,
        PanelWidgetSection::Media,
        PanelWidgetSection::Toggles,
        PanelWidgetSection::Stats,
        PanelWidgetSection::Cards,
    ]
}
