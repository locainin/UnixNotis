use std::collections::HashSet;

use super::{MAX_WIDGET_COLUMNS, MIN_WIDGET_COLUMNS};
use crate::{default_panel_widget_order, Config, PanelConfig, PanelWidgetSection};

pub(super) fn sanitize_panel_text(panel: &mut PanelConfig) {
    // Empty core labels make the panel harder to operate, so restore only required text
    if panel.title.trim().is_empty() {
        panel.title = PanelConfig::default().title;
    }
    if panel.clear_label.trim().is_empty() {
        panel.clear_label = PanelConfig::default().clear_label;
    }
}

pub(super) fn sanitize_panel_widget_order(order: &mut Vec<PanelWidgetSection>) {
    if order.is_empty() {
        // An empty order means "use the stock order", not "render no widget sections"
        *order = default_panel_widget_order();
        return;
    }

    let mut seen = HashSet::new();
    // Keep the first occurrence so user intent survives when duplicates are present
    order.retain(|section| seen.insert(*section));
    for section in default_panel_widget_order() {
        if !seen.contains(&section) {
            // Missing sections are appended so future defaults remain reachable after upgrades
            order.push(section);
        }
    }
}

pub(super) fn sanitize_widget_columns(config: &mut Config) {
    let defaults = crate::WidgetsConfig::default();
    config.widgets.toggle_columns =
        sanitize_column_count(config.widgets.toggle_columns, defaults.toggle_columns);
    config.widgets.stat_columns =
        sanitize_column_count(config.widgets.stat_columns, defaults.stat_columns);
    config.widgets.card_columns =
        sanitize_column_count(config.widgets.card_columns, defaults.card_columns);
}

fn sanitize_column_count(value: usize, default_value: usize) -> usize {
    if value == 0 {
        // Zero is treated as "auto/default" because it cannot form a usable grid
        return default_value;
    }
    value.clamp(MIN_WIDGET_COLUMNS, MAX_WIDGET_COLUMNS)
}
