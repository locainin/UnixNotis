//! Config reload and widget rebuild logic for `UiState`
//!
//! Keeps dynamic configuration changes isolated from event handling and
//! visibility logic

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use gtk::prelude::*;
use tracing::debug;
use unixnotis_core::{
    css::hooks, Config, ConfigDiagnostic, ConfigError, PanelDebugLevel, PanelWidgetSection,
    ThemePaths,
};
use unixnotis_ui::css::CssReloadReport;

use super::list;
use super::panel::notification_header_row_visible;
use super::widget_builders::{build_extra_widgets, build_quick_controls, clear_container};
use super::{panel, UiState};

struct ReloadInputs {
    config: Config,
    diagnostics: Vec<ConfigDiagnostic>,
    theme_paths: ThemePaths,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReloadNoticeKind {
    Config,
    Css,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ReloadNoticeFingerprint {
    kind: ReloadNoticeKind,
    identity: String,
}

#[derive(Debug)]
pub(super) enum ReloadFailure {
    Config(ConfigError),
    ThemeBase(String),
    ThemePaths(String),
}

#[derive(Debug)]
pub(super) enum ConfigReloadOutcome {
    Applied {
        diagnostics: Vec<ConfigDiagnostic>,
        css: CssReloadReport,
    },
    Rejected {
        failure: ReloadFailure,
    },
}

impl UiState {
    pub(super) fn reload_config(&mut self) -> ConfigReloadOutcome {
        let reload = match self.load_reload_inputs() {
            Ok(reload) => reload,
            Err(failure) => {
                // Log only the stable category because parser errors can contain config text
                tracing::warn!(kind = failure.kind(), "failed to reload config");
                self.show_config_reload_failure(&failure);
                return ConfigReloadOutcome::Rejected { failure };
            }
        };
        let widgets_changed = self.config.widgets != reload.config.widgets;

        // Store the new config early so shared helpers see one consistent state
        self.config = reload.config.clone();
        debug!("config reloaded");

        let css = self.apply_reloaded_theme(&reload);
        self.apply_reloaded_panel(&reload.config);
        // Media depends on panel geometry, so it needs the new width before widgets rebuild
        self.apply_media_config(&reload.config);
        self.apply_widget_sections_after_reload(&reload.config, widgets_changed);
        self.apply_list_config_after_reload(&reload.config);
        self.finish_reload_runtime(&reload.config);
        // Any accepted config replaces a prior rejection before CSS reports its own result
        self.clear_reload_notice();
        self.apply_css_reload_notice(&css);
        ConfigReloadOutcome::Applied {
            diagnostics: reload.diagnostics,
            css,
        }
    }

    fn load_reload_inputs(&self) -> Result<ReloadInputs, ReloadFailure> {
        // The accepted report keeps diagnostics tied to the same config object being applied
        let report =
            Config::load_from_path_with_report(&self.config_path).map_err(ReloadFailure::Config)?;
        unixnotis_core::log_config_diagnostics(&report.diagnostics);
        let config = report.config;
        let theme_base = match Config::config_dir_for_path(&self.config_path) {
            Ok(path) => path,
            Err(err) => return Err(ReloadFailure::ThemeBase(err.to_string())),
        };
        let theme_paths = match config.resolve_theme_paths_from(&theme_base) {
            Ok(paths) => paths,
            Err(err) => return Err(ReloadFailure::ThemePaths(err.to_string())),
        };

        Ok(ReloadInputs {
            config,
            diagnostics: report.diagnostics,
            theme_paths,
        })
    }

    fn apply_reloaded_theme(&mut self, reload: &ReloadInputs) -> CssReloadReport {
        self.css
            .update_theme(reload.theme_paths.clone(), reload.config.theme.clone());
        let report = self.css.reload(unixnotis_ui::css::DEFAULT_CSS);
        // New theme assets may replace old cache misses, so clear the miss cache now
        self.icon_resolver.clear_missing_cache();
        report
    }

    pub(super) fn reload_css(&mut self) -> CssReloadReport {
        let report = self.css.reload(unixnotis_ui::css::DEFAULT_CSS);
        self.apply_css_reload_notice(&report);
        report
    }

    fn show_config_reload_failure(&mut self, failure: &ReloadFailure) {
        let detail = match failure {
            ReloadFailure::Config(error) => error.shareable_summary(),
            ReloadFailure::ThemeBase(detail) | ReloadFailure::ThemePaths(detail) => detail,
        };
        let detail = unixnotis_core::util::sanitize_inline_display_text(detail);
        let message =
            format!("Config reload rejected\nThe previous configuration is still active\n{detail}");
        let identity = failure.fingerprint();
        self.show_reload_notice_with_identity(ReloadNoticeKind::Config, &message, true, &identity);
    }

    fn apply_css_reload_notice(&mut self, report: &CssReloadReport) {
        // Intentional empty files are valid fallback requests and do not produce a notice
        let failures = report.read_failures().collect::<Vec<_>>();
        if failures.is_empty() {
            if self
                .last_reload_notice
                .as_ref()
                .is_some_and(|notice| notice.kind == ReloadNoticeKind::Css)
            {
                self.clear_reload_notice();
            }
            return;
        }
        let first = failures[0];
        // File names are sufficient for the panel and avoid exposing full account paths
        let file = first
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("CSS file");
        let suffix = if failures.len() == 1 {
            String::new()
        } else {
            format!(" and {} other layer(s)", failures.len() - 1)
        };
        let message = format!(
            "Theme fallback active\n{file}{suffix} could not be read; embedded styling is active"
        );
        self.show_reload_notice(ReloadNoticeKind::Css, &message, false);
    }

    fn show_reload_notice(&mut self, kind: ReloadNoticeKind, message: &str, error: bool) {
        self.show_reload_notice_with_identity(kind, message, error, message);
    }

    fn show_reload_notice_with_identity(
        &mut self,
        kind: ReloadNoticeKind,
        message: &str,
        error: bool,
        identity: &str,
    ) {
        let fingerprint = ReloadNoticeFingerprint {
            kind,
            identity: identity.to_string(),
        };
        if self.last_reload_notice.as_ref() == Some(&fingerprint) {
            // Watcher bursts must not reopen a notice that was already dismissed
            return;
        }
        // Store identity before revealing so dismissal belongs to this exact failure
        self.last_reload_notice = Some(fingerprint);
        self.panel.reload_notice_label.set_label(message);
        self.panel
            .reload_notice_shell
            .remove_css_class(hooks::panel_shell::RELOAD_NOTICE_ERROR);
        self.panel
            .reload_notice_shell
            .remove_css_class(hooks::panel_shell::RELOAD_NOTICE_WARNING);
        self.panel.reload_notice_shell.add_css_class(if error {
            hooks::panel_shell::RELOAD_NOTICE_ERROR
        } else {
            hooks::panel_shell::RELOAD_NOTICE_WARNING
        });
        self.panel.reload_notice_revealer.set_reveal_child(true);
    }

    fn clear_reload_notice(&mut self) {
        self.last_reload_notice = None;
        self.panel.reload_notice_revealer.set_reveal_child(false);
    }

    pub(in crate::ui) fn apply_reloaded_panel(&mut self, config: &Config) {
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
        panel::apply_widget_density(
            &self.panel.widget_stack,
            &self.panel.quick_controls,
            &self.panel.media_container,
            config.widgets.density,
        );
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
                PanelWidgetSection::Media => self.panel.media_container.clone().upcast(),
                PanelWidgetSection::Toggles => self.panel.toggle_container.clone().upcast(),
                PanelWidgetSection::Sliders => self.panel.quick_controls.clone().upcast(),
                PanelWidgetSection::Stats => self.panel.stat_container.clone().upcast(),
                PanelWidgetSection::Cards => self.panel.card_container.clone().upcast(),
            };
            self.panel
                .widget_stack
                .reorder_child_after(&child, previous.as_ref());
            // The next child is inserted after the child placed in this iteration
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

    pub(in crate::ui) fn apply_list_config_after_reload(&mut self, config: &Config) {
        // A compact value object prevents the list from reading half-applied UI state
        let list_config = list::NotificationListConfig {
            max_active: config.history.max_active,
            max_entries: config.history.max_entries,
            transient_to_history: config.history.transient_to_history,
            show_notification_metadata: config.panel.notification_metadata_visible,
            show_notification_thumbnails: config.panel.notification_thumbnails_visible,
            empty_text: config.panel.empty_text.clone(),
            empty_offset_top: config.panel.empty_offset_top,
            empty_alignment: config.panel.empty_alignment,
        };
        self.list.apply_config(&list_config);
        // Empty-state placement depends on both list settings and current widget visibility
        self.set_widgets_collapsed(self.widgets_collapsed);
    }

    fn finish_reload_runtime(&mut self, config: &Config) {
        // Refresh timers may need new intervals even when widget structure is unchanged
        self.restart_refresh_timer();
        if config.panel.respect_work_area {
            // Clearing the cache prevents stale compositor margins from surviving reload
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
        let (toggles, stats, cards) =
            build_extra_widgets(&self.panel, config, &self.widget_icon_resolver);
        // Replace all handles together after the containers hold the new children
        self.toggles = toggles;
        self.stats = stats;
        self.cards = cards;
    }
}

impl ReloadFailure {
    pub(super) const fn kind(&self) -> &'static str {
        match self {
            Self::Config(_) => "config",
            Self::ThemeBase(_) => "theme-base",
            Self::ThemePaths(_) => "theme-paths",
        }
    }

    fn fingerprint(&self) -> String {
        // Hash private parser details so distinct failures remain distinguishable without display
        let mut hasher = DefaultHasher::new();
        format!("{self:?}").hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

#[cfg(test)]
#[path = "tests/config.rs"]
mod tests;
