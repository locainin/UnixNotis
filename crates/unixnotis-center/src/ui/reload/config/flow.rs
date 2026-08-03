//! Reload input loading and top-level application flow

use tracing::debug;
use unixnotis_core::{Config, ConfigDiagnostic, ThemePaths};
use unixnotis_ui::css::CssReloadReport;

use super::outcome::{ConfigReloadOutcome, ReloadFailure};
use crate::ui::reload::notices::ReloadNoticeKind;
use crate::ui::UiState;

struct ReloadInputs {
    config: Config,
    diagnostics: Vec<ConfigDiagnostic>,
    theme_paths: ThemePaths,
}

impl UiState {
    pub(in crate::ui) fn reload_config(&mut self) -> ConfigReloadOutcome {
        self.capture_notice_dismissal();
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
        self.clear_reload_notice(ReloadNoticeKind::Config);
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

    pub(in crate::ui) fn reload_css(&mut self) -> CssReloadReport {
        self.capture_notice_dismissal();
        let report = self.css.reload(unixnotis_ui::css::DEFAULT_CSS);
        self.apply_css_reload_notice(&report);
        report
    }
}
