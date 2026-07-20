//! Widget, list, and refresh updates applied after configuration reload

use tracing::debug;
use unixnotis_core::Config;

use crate::ui::notifications;
use crate::ui::widget_builders::{build_extra_widgets, build_quick_controls, clear_container};
use crate::ui::UiState;

impl UiState {
    pub(super) fn apply_widget_sections_after_reload(
        &mut self,
        config: &Config,
        widgets_changed: bool,
    ) {
        if widgets_changed {
            // Widget rebuilds are the expensive part, so skip them when structure is unchanged
            self.apply_widget_config(config);
        } else {
            debug!("widget config unchanged; skipping rebuild");
        }
    }

    pub(in crate::ui) fn apply_list_config_after_reload(&mut self, config: &Config) {
        // Menu inputs and typed deadlines are live configuration like the surrounding actions
        self.dnd_duration_menu.apply_config(&config.panel);
        // A compact value object prevents the list from reading half-applied UI state
        let list_config = notifications::NotificationListConfig {
            max_active: config.history.max_active,
            max_entries: config.history.max_entries,
            transient_to_history: config.history.transient_to_history,
            show_notification_metadata: config.panel.notification_metadata_visible,
            notification_metadata: config.panel.notification_metadata.clone(),
            notification_corners: config.theme.notification_corners,
            show_notification_thumbnails: config.panel.notification_thumbnails_visible,
            empty_text: config.panel.empty_text.clone(),
            empty_offset_top: config.panel.empty_offset_top,
            empty_alignment: config.panel.empty_alignment,
        };
        self.list.apply_config(&list_config);
        // Empty-state placement depends on both list settings and current widget visibility
        self.set_widgets_collapsed(self.widgets_collapsed);
    }

    pub(super) fn finish_reload_runtime(&mut self, config: &Config) {
        // Refresh timers may need new intervals even when widget structure is unchanged
        self.restart_refresh_timer();
        if config.panel.respect_work_area {
            // Clearing the cache prevents stale compositor margins from surviving reload
            self.work_area = None;
            // Work area is refreshed after reload so compositor margins can update one more time
            crate::ui::hyprland::refresh_reserved_work_area(
                config.panel.output.clone(),
                self.event_tx.clone(),
            );
        }
    }

    fn apply_widget_config(&mut self, config: &Config) {
        // Old children are cleared first so the rebuild can treat each section as fresh state
        clear_container(&self.panel.sections.quick_controls);
        let (volume, brightness) = build_quick_controls(&self.panel, config);
        self.volume = volume;
        self.brightness = brightness;
        clear_container(&self.panel.sections.toggle_container);
        clear_container(&self.panel.sections.stat_container);
        clear_container(&self.panel.sections.card_container);
        let (toggles, stats, cards) =
            build_extra_widgets(&self.panel, config, &self.widget_icon_resolver);
        // Replace all handles together after the containers hold the new children
        self.toggles = toggles;
        self.stats = stats;
        self.cards = cards;
    }
}
