//! Config reload and widget rebuild logic for `UiState`.
//!
//! Keeps dynamic configuration changes isolated from event handling and
//! visibility logic.

use gtk::prelude::*;
use tracing::debug;
use unixnotis_core::{
    css::hooks, Config, PanelClearButtonPlacement, PanelDebugLevel, PanelWidgetSection, ThemePaths,
};

use super::list;
use super::panel::notification_header_row_visible;
use super::widget_builders::{build_extra_widgets, build_quick_controls, clear_container};
use super::{panel, UiState};

struct ReloadInputs {
    config: Config,
    theme_paths: ThemePaths,
}

impl UiState {
    pub(super) fn reload_config(&mut self) {
        let Some(reload) = self.load_reload_inputs() else {
            return;
        };
        let widgets_changed = self.config.widgets != reload.config.widgets;

        // Store the new config early so shared helpers see one consistent state
        self.config = reload.config.clone();
        debug!("config reloaded");

        self.apply_reloaded_theme(&reload);
        self.apply_reloaded_panel(&reload.config);
        // Media depends on panel geometry, so it needs the new width before widgets rebuild
        self.apply_media_config(&reload.config);
        self.apply_widget_sections_after_reload(&reload.config, widgets_changed);
        self.apply_list_config_after_reload(&reload.config);
        self.finish_reload_runtime(&reload.config);
    }

    fn load_reload_inputs(&self) -> Option<ReloadInputs> {
        let config = match Config::load_from_path(&self.config_path) {
            Ok(config) => config,
            Err(err) => {
                // Reload failures keep the old state so the live panel stays usable
                tracing::warn!(?err, "failed to reload config");
                return None;
            }
        };
        let theme_base = match Config::config_dir_for_path(&self.config_path) {
            Ok(path) => path,
            Err(err) => {
                // Theme lookup needs a stable base dir before any CSS path work starts
                tracing::warn!(?err, "failed to resolve config dir");
                return None;
            }
        };
        let theme_paths = match config.resolve_theme_paths_from(&theme_base) {
            Ok(paths) => paths,
            Err(err) => {
                // Theme path errors should not partially swap the running config
                tracing::warn!(?err, "failed to resolve theme paths");
                return None;
            }
        };

        Some(ReloadInputs {
            config,
            theme_paths,
        })
    }

    fn apply_reloaded_theme(&mut self, reload: &ReloadInputs) {
        self.css
            .update_theme(reload.theme_paths.clone(), reload.config.theme.clone());
        self.css.reload(unixnotis_ui::css::DEFAULT_CSS);
        // New theme assets may replace old cache misses, so clear the miss cache now
        self.icon_resolver.clear_missing_cache();
    }

    fn apply_reloaded_panel(&mut self, config: &Config) {
        // Geometry goes first so later sections can size themselves from the final panel width
        panel::apply_panel_config(&self.panel, config, self.work_area);
        self.panel.header_title.set_label(&config.panel.title);
        self.panel.header_subtitle.set_label(&config.panel.subtitle);
        self.panel
            .header_subtitle
            .set_visible(!config.panel.subtitle.is_empty());
        self.panel
            .search_entry
            .set_placeholder_text(Some(&config.panel.search_placeholder));
        self.panel
            .search_revealer
            .set_reveal_child(config.panel.search_visible || self.panel.search_toggle.is_active());
        self.panel
            .header_action_row
            .set_visible(config.panel.action_row_visible);
        panel::apply_reloaded_panel_chrome(&self.panel, &config.panel);
        self.panel
            .notification_header
            .set_label(&config.panel.recent_notifications_label);
        self.panel.notification_header.set_visible(
            config.panel.notification_section_visible
                && !config.panel.recent_notifications_label.is_empty(),
        );
        self.panel
            .notification_header_row
            .set_visible(notification_header_row_visible(&config.panel));
        self.update_section_header(
            &self.panel.toggle_section_header,
            &config.panel.quick_actions_label,
        );
        self.update_section_header(
            &self.panel.stat_section_header,
            &config.panel.system_status_label,
        );
        if config.panel.notification_section_visible {
            self.panel
                .notification_container
                .add_css_class(hooks::panel_shell::RECENT_SECTION);
        } else {
            self.panel
                .notification_container
                .remove_css_class(hooks::panel_shell::RECENT_SECTION);
        }
        self.panel
            .scroller
            .set_vexpand(config.panel.notification_list_expand);
        self.panel
            .notification_container
            .set_vexpand(config.panel.notification_list_expand);
        panel::apply_reloaded_body_order(&self.panel, &config.panel.section_order);
        self.apply_widget_order(&config.panel.widget_order);
        self.update_clear_button_visibility(config);
        self.panel
            .footer_label
            .set_label(&config.panel.footer_label);
        self.panel
            .footer_label
            .set_visible(!config.panel.footer_label.is_empty());
        self.log_debug(PanelDebugLevel::Info, || {
            "panel config applied after reload".to_string()
        });
    }

    fn update_clear_button_visibility(&self, config: &Config) {
        self.panel.clear_action_button.set_visible(matches!(
            config.panel.clear_button_placement,
            PanelClearButtonPlacement::ActionRow
        ));
        self.panel.clear_header_button.set_visible(matches!(
            config.panel.clear_button_placement,
            PanelClearButtonPlacement::NotificationHeader
        ));
    }

    fn update_section_header(&self, header: &gtk::Label, label: &str) {
        // Section headers are built once and updated in place on config reload
        header.set_label(label);
        header.set_visible(!label.is_empty());
    }

    fn apply_widget_order(&self, order: &[PanelWidgetSection]) {
        let mut previous: Option<gtk::Widget> = None;
        for section in order {
            let child: gtk::Widget = match section {
                PanelWidgetSection::Media => self.panel.media_container.clone().upcast(),
                PanelWidgetSection::Toggles => self.panel.toggle_container.clone().upcast(),
                PanelWidgetSection::Sliders => self.panel.quick_controls.clone().upcast(),
                PanelWidgetSection::Stats => self.panel.stat_container.clone().upcast(),
                PanelWidgetSection::Cards => self.panel.card_container.clone().upcast(),
            };
            self.panel
                .widget_stack
                .reorder_child_after(&child, previous.as_ref());
            previous = Some(child);
        }
    }

    fn apply_widget_sections_after_reload(&mut self, config: &Config, widgets_changed: bool) {
        if widgets_changed {
            // Widget rebuilds are the expensive part, so skip them when structure is unchanged
            self.apply_widget_config(config);
        } else {
            debug!("widget config unchanged; skipping rebuild");
        }
    }

    fn apply_list_config_after_reload(&mut self, config: &Config) {
        let list_config = list::NotificationListConfig {
            max_active: config.history.max_active,
            max_entries: config.history.max_entries,
            transient_to_history: config.history.transient_to_history,
            show_notification_metadata: config.panel.notification_metadata_visible,
            show_notification_thumbnails: config.panel.notification_thumbnails_visible,
            empty_text: config.panel.empty_text.clone(),
            empty_offset_top: config.panel.empty_offset_top,
        };
        let has_widgets = !self.widgets_collapsed && self.has_any_widgets();
        self.list.apply_config(&list_config, has_widgets);
        self.set_widgets_collapsed(self.widgets_collapsed);
    }

    fn finish_reload_runtime(&mut self, config: &Config) {
        // Refresh timers may need new intervals even when widget structure is unchanged
        self.restart_refresh_timer();
        if config.panel.respect_work_area {
            self.work_area = None;
            // Work area is refreshed after reload so compositor margins can update one more time
            super::hyprland::refresh_reserved_work_area(
                config.panel.output.clone(),
                self.event_tx.clone(),
            );
        }
    }

    fn apply_widget_config(&mut self, config: &Config) {
        // Old children are cleared first so the rebuild can treat each section as fresh state
        clear_container(&self.panel.quick_controls);
        let (volume, brightness) = build_quick_controls(&self.panel, config);
        self.volume = volume;
        self.brightness = brightness;
        clear_container(&self.panel.toggle_container);
        clear_container(&self.panel.stat_container);
        clear_container(&self.panel.card_container);
        let (toggles, stats, cards) = build_extra_widgets(&self.panel, config);
        self.toggles = toggles;
        self.stats = stats;
        self.cards = cards;
    }
}
