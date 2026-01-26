//! Config reload and widget rebuild logic for `UiState`.
//!
//! Keeps dynamic configuration changes isolated from event handling and
//! visibility logic.

use gtk::prelude::*;
use tracing::debug;
use unixnotis_core::{Config, PanelDebugLevel};

use super::list;
use super::widget_builders::{build_extra_widgets, build_quick_controls, clear_container};
use super::{media_widget, panel, UiState};

impl UiState {
    pub(super) fn reload_config(&mut self) {
        let widgets_before = self.config.widgets.clone();
        let config = match Config::load_from_path(&self.config_path) {
            Ok(config) => config,
            Err(err) => {
                // Failed reload keeps the previous config to avoid half-applied state.
                tracing::warn!(?err, "failed to reload config");
                return;
            }
        };
        let theme_base = self
            .config_path
            .parent()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                Config::default_config_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            });
        let theme_paths = match config.resolve_theme_paths_from(&theme_base) {
            Ok(paths) => paths,
            Err(err) => {
                // Theme path errors should not discard the current config.
                tracing::warn!(?err, "failed to resolve theme paths");
                return;
            }
        };

        self.config = config.clone();
        debug!("config reloaded");
        self.css.update_theme(theme_paths, config.theme.clone());
        self.css.reload(unixnotis_ui::css::DEFAULT_CSS);
        panel::apply_panel_config(&self.panel, &config, self.work_area);
        self.log_debug(PanelDebugLevel::Info, || {
            "panel config applied after reload".to_string()
        });
        // Media widgets depend on panel geometry, so update them before widgets rebuild.
        self.apply_media_config(&config);
        if config.widgets != widgets_before {
            // Only rebuild widgets when config structure changes to reduce churn.
            self.apply_widget_config(&config);
        } else {
            debug!("widget config unchanged; skipping rebuild");
        }
        let list_config = list::NotificationListConfig {
            max_active: config.history.max_active,
            max_entries: config.history.max_entries,
            empty_text: config.panel.empty_text.clone(),
            empty_offset_top: config.panel.empty_offset_top,
        };
        let has_widgets = self.panel.quick_controls.is_visible()
            || self.panel.media_container.is_visible()
            || self.panel.toggle_container.is_visible()
            || self.panel.stat_container.is_visible()
            || self.panel.card_container.is_visible();
        self.list.apply_config(&list_config, has_widgets);
        self.restart_refresh_timer();
        if config.panel.respect_work_area {
            self.work_area = None;
            // Refreshing work area ensures the panel snaps to the latest compositor state.
            super::hyprland::refresh_reserved_work_area(
                config.panel.output.clone(),
                self.event_tx.clone(),
            );
        }
    }

    fn apply_media_config(&mut self, config: &Config) {
        if !config.media.enabled {
            self.panel.media_container.set_visible(false);
            self.clear_media_container();
            self.media = None;
            debug!("media disabled");
            return;
        }

        self.panel.media_container.set_visible(true);
        match (self.media.as_mut(), self.media_handle.as_ref()) {
            (Some(media), _) => {
                // Reuse existing widget to avoid extra allocations during reloads.
                debug!("media layout updated");
                media.apply_layout(config.panel.width, config.media.title_char_limit);
            }
            (None, Some(handle)) => {
                debug!("media widget created");
                let media = media_widget::MediaWidget::new(
                    &self.panel.media_container,
                    handle.clone(),
                    config.panel.width,
                    config.media.title_char_limit,
                );
                self.media = Some(media);
            }
            (None, None) => {
                // Media handle comes from the shared runtime; missing handle needs restart.
                tracing::warn!("media runtime not available; restart required to enable media");
            }
        }
    }

    fn apply_widget_config(&mut self, config: &Config) {
        // Clear containers before rebuilding to avoid stale widgets and duplicated rows.
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

    fn clear_media_container(&self) {
        while let Some(child) = self.panel.media_container.first_child() {
            self.panel.media_container.remove(&child);
        }
    }
}
