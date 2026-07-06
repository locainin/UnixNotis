//! Panel reload helpers for structure and action chrome

use unixnotis_core::{PanelConfig, PanelSection};

use super::types::PanelWidgets;

pub(crate) fn apply_reloaded_panel_chrome(panel: &PanelWidgets, config: &PanelConfig) {
    super::actions::apply_panel_action_config(
        &panel.header_top,
        &super::actions::PanelActionWidgets {
            group: panel.header_action_group.clone(),
            focus_toggle: panel.focus_toggle.clone(),
            dnd_toggle: panel.dnd_toggle.clone(),
            clear_button: panel.clear_action_button.clone(),
            search_toggle: panel.search_toggle.clone(),
            close_button: panel.close_button.clone(),
        },
        config,
    );
    super::actions::apply_clear_button_config(&panel.clear_header_button, config);
}

pub(crate) fn apply_reloaded_body_order(panel: &PanelWidgets, order: &[PanelSection]) {
    super::sections::apply_panel_body_section_order(
        &panel.body_stack,
        &panel.widget_revealer,
        &panel.notification_container,
        order,
    );
}

#[cfg(test)]
#[path = "tests/reload.rs"]
mod tests;
