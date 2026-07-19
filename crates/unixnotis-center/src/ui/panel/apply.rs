//! Panel reload helpers for structure and action chrome

use unixnotis_core::{PanelConfig, PanelSection};

use super::widgets::PanelWidgets;

pub fn apply_reloaded_panel_chrome(panel: &PanelWidgets, config: &PanelConfig) {
    super::header::actions::apply_panel_action_config(
        &panel.header.top,
        &panel.header.actions,
        config,
    );
    super::header::actions::apply_clear_button_config(&panel.sections.clear_header_button, config);
}

pub fn apply_reloaded_body_order(panel: &PanelWidgets, order: &[PanelSection]) {
    super::body::apply_panel_body_section_order(
        &panel.sections.body_stack,
        &panel.sections.widget_revealer,
        &panel.sections.notification_container,
        order,
    );
}

#[cfg(test)]
#[path = "tests/apply.rs"]
mod tests;
