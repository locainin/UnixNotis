//! Panel presentation updates applied after a configuration reload

use gtk::prelude::*;
use unixnotis_core::{css::hooks, Config, PanelDebugLevel, PanelWidgetSection};

use crate::ui::{panel, UiState};

impl UiState {
    pub(in crate::ui) fn apply_reloaded_panel(&mut self, config: &Config) {
        // Geometry goes first so later sections can size themselves from the final panel width
        panel::geometry::apply_panel_config(&self.panel, config, self.work_area);
        self.panel.header.title.set_label(&config.panel.title);
        self.panel.header.subtitle.set_label(&config.panel.subtitle);
        self.panel
            .header
            .subtitle
            .set_visible(!config.panel.subtitle.is_empty());
        self.panel
            .header
            .search
            .entry
            .set_placeholder_text(Some(&config.panel.search_placeholder));
        self.panel
            .header
            .search
            .magnifier
            .set_icon_name(Some(&config.panel.search_magnifier_icon));
        self.panel
            .header
            .search
            .clear_button
            .set_visible(!self.panel.header.search.entry.text().is_empty());
        let search_open =
            config.panel.search_visible || self.panel.header.actions.search_toggle.is_active();
        panel::header::search::set_search_open(
            &self.panel.header.actions.search_toggle,
            &self.panel.header.search.revealer,
            &self.panel.header.search.entry,
            self.search_toggle_guard.as_ref(),
            search_open,
        );
        self.panel
            .header
            .action_row
            .set_visible(config.panel.action_row_visible);
        panel::apply::apply_reloaded_panel_chrome(&self.panel, &config.panel);
        self.panel
            .sections
            .notification_header
            .set_label(&config.panel.recent_notifications_label);
        self.panel.sections.notification_header.set_visible(
            config.panel.notification_section_visible
                && !config.panel.recent_notifications_label.is_empty(),
        );
        self.panel
            .sections
            .notification_header_row
            .set_visible(panel::body::notification_header_row_visible(&config.panel));
        self.update_section_header(
            &self.panel.sections.toggle_section_header,
            &config.panel.quick_actions_label,
        );
        self.update_section_header(
            &self.panel.sections.stat_section_header,
            &config.panel.system_status_label,
        );
        if config.panel.notification_section_visible {
            self.panel
                .sections
                .notification_container
                .add_css_class(hooks::panel_shell::RECENT_SECTION);
        } else {
            self.panel
                .sections
                .notification_container
                .remove_css_class(hooks::panel_shell::RECENT_SECTION);
        }
        self.panel
            .sections
            .scroller
            .set_vexpand(config.panel.notification_list_expand);
        self.panel
            .sections
            .notification_container
            .set_vexpand(config.panel.notification_list_expand);
        panel::apply::apply_reloaded_body_order(&self.panel, &config.panel.section_order);
        self.apply_widget_order(&config.panel.widget_order);
        panel::body::apply_widget_density(
            &self.panel.sections.widget_stack,
            &self.panel.sections.quick_controls,
            &self.panel.sections.media_container,
            config.widgets.density,
        );
        self.panel
            .sections
            .footer
            .set_label(&config.panel.footer_label);
        self.panel
            .sections
            .footer
            .set_visible(!config.panel.footer_label.is_empty());
        self.log_debug(PanelDebugLevel::Info, || {
            "panel config applied after reload".to_string()
        });
    }

    fn update_section_header(&self, header: &gtk::Label, label: &str) {
        // Section headers are built once and updated in place on config reload
        header.set_label(label);
        header.set_visible(!label.is_empty());
    }

    fn apply_widget_order(&self, order: &[PanelWidgetSection]) {
        let mut previous: Option<gtk::Widget> = None;
        for section in order {
            // Config enum values map to the long-lived container built at startup
            let child: gtk::Widget = match section {
                PanelWidgetSection::Media => self.panel.sections.media_container.clone().upcast(),
                PanelWidgetSection::Toggles => {
                    self.panel.sections.toggle_container.clone().upcast()
                }
                PanelWidgetSection::Sliders => self.panel.sections.quick_controls.clone().upcast(),
                PanelWidgetSection::Stats => self.panel.sections.stat_container.clone().upcast(),
                PanelWidgetSection::Cards => self.panel.sections.card_container.clone().upcast(),
            };
            self.panel
                .sections
                .widget_stack
                .reorder_child_after(&child, previous.as_ref());
            // The next child is inserted after the child placed in this iteration
            previous = Some(child);
        }
    }
}
