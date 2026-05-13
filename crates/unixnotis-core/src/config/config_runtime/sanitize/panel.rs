use std::collections::HashSet;

use super::{
    default_panel_widget_order, Config, PanelConfig, PanelWidgetSection, MAX_WIDGET_COLUMNS,
    MIN_WIDGET_COLUMNS,
};

pub(super) fn sanitize_panel_text(panel: &mut PanelConfig) {
    if panel.title.trim().is_empty() {
        panel.title = PanelConfig::default().title;
    }
    if panel.clear_label.trim().is_empty() {
        panel.clear_label = PanelConfig::default().clear_label;
    }
}

pub(super) fn sanitize_panel_widget_order(order: &mut Vec<PanelWidgetSection>) {
    if order.is_empty() {
        *order = default_panel_widget_order();
        return;
    }

    let mut seen = HashSet::new();
    order.retain(|section| seen.insert(*section));
    for section in default_panel_widget_order() {
        if !seen.contains(&section) {
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
        return default_value;
    }
    value.clamp(MIN_WIDGET_COLUMNS, MAX_WIDGET_COLUMNS)
}
