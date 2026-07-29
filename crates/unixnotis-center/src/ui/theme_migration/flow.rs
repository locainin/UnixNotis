//! Stock theme migration state transitions

use tracing::{debug, warn};
use unixnotis_core::{
    apply_stock_theme_migration, detect_stock_theme_migration, keep_current_stock_theme, Config,
    ConfigError, StockThemeMigration, ThemePaths,
};
use unixnotis_ui::css::CssReloadReport;

use crate::ui::reload::ReloadNoticeKind;
use crate::ui::UiState;

impl UiState {
    pub(in crate::ui) fn initialize_stock_theme_migration(&mut self) {
        self.refresh_stock_theme_migration_notice();
    }

    pub(in crate::ui) fn preview_stock_theme_migration(&mut self) {
        let Some(migration) = self.theme_migration.clone() else {
            return;
        };
        let Ok(configured_paths) = self.configured_theme_paths() else {
            self.show_theme_migration_failure(
                &migration,
                "Preview unavailable\nThe configured theme location could not be resolved",
            );
            return;
        };
        let preview_paths = match migration.preview_paths(&configured_paths) {
            Ok(paths) => paths,
            Err(error) => {
                warn!(?error, "stock theme preview rejected");
                self.show_theme_migration_failure(
                    &migration,
                    "Preview unavailable\nVerified staged theme files could not be loaded",
                );
                return;
            }
        };

        // Only the center provider paths change during preview; active files remain untouched
        self.css
            .update_theme(preview_paths, self.config.theme.clone());
        let report = self.css.reload(unixnotis_ui::css::DEFAULT_CSS);
        self.icon_resolver.clear_missing_cache();
        self.theme_preview_active = true;
        self.apply_css_reload_notice(&report);
        self.show_theme_migration_notice(
            &migration,
            &format!(
                "Stock theme preview active\nReview the {} layer(s), then Apply or Keep Current",
                migration.layer_summary()
            ),
            false,
        );
    }

    pub(in crate::ui) fn apply_stock_theme_migration(&mut self) {
        let Some(migration) = self.theme_migration.clone() else {
            return;
        };
        let configured_paths = match self.configured_theme_paths() {
            Ok(paths) => paths,
            Err(error) => {
                warn!(?error, "stock theme Apply path resolution failed");
                self.show_theme_migration_failure(
                    &migration,
                    "Theme update was not applied\nThe configured theme location could not be resolved",
                );
                return;
            }
        };

        match apply_stock_theme_migration(&configured_paths, &migration) {
            Ok(report) => {
                debug!(
                    updated_layers = report.updated_layers,
                    "approved stock theme update applied"
                );
                let css = self.restore_configured_theme(configured_paths);
                self.theme_migration = None;
                self.clear_reload_notice(ReloadNoticeKind::ThemeMigration);
                self.apply_css_reload_notice(&css);
            }
            Err(error) => {
                warn!(?error, "approved stock theme update stopped");
                let css = self.restore_configured_theme(configured_paths);
                self.apply_css_reload_notice(&css);
                self.show_theme_migration_failure(
                    &migration,
                    "Theme update stopped safely\nA file changed or could not be backed up; current files remain active",
                );
            }
        }
    }

    pub(in crate::ui) fn keep_current_stock_theme(&mut self) {
        let Some(migration) = self.theme_migration.clone() else {
            return;
        };
        let configured_paths = match self.configured_theme_paths() {
            Ok(paths) => paths,
            Err(error) => {
                warn!(?error, "Keep Current path resolution failed");
                self.show_theme_migration_failure(
                    &migration,
                    "Keep Current could not be saved\nThe configured theme location could not be resolved",
                );
                return;
            }
        };

        let outcome = keep_current_stock_theme(&configured_paths);
        let css = self.restore_configured_theme(configured_paths);
        self.apply_css_reload_notice(&css);
        match outcome {
            Ok(()) => {
                self.theme_migration = None;
                self.clear_reload_notice(ReloadNoticeKind::ThemeMigration);
            }
            Err(error) => {
                warn!(?error, "Keep Current choice could not be persisted");
                self.show_theme_migration_failure(
                    &migration,
                    "Keep Current could not be saved\nNo theme file was changed; try again after checking config permissions",
                );
            }
        }
    }

    pub(in crate::ui) fn refresh_stock_theme_migration_notice(&mut self) {
        let paths = match self.configured_theme_paths() {
            Ok(paths) => paths,
            Err(error) => {
                warn!(?error, "stock theme migration path resolution failed");
                self.theme_migration = None;
                self.clear_reload_notice(ReloadNoticeKind::ThemeMigration);
                return;
            }
        };
        match detect_stock_theme_migration(&paths) {
            Ok(Some(migration)) => {
                let message = format!(
                    "Stock theme update available\nPreview UnixNotis {} for the {} layer(s) before applying",
                    env!("CARGO_PKG_VERSION"),
                    migration.layer_summary()
                );
                self.theme_migration = Some(migration.clone());
                self.show_theme_migration_notice(&migration, &message, false);
            }
            Ok(None) => {
                self.theme_migration = None;
                self.clear_reload_notice(ReloadNoticeKind::ThemeMigration);
            }
            Err(error) => {
                // An unsafe marker or unreadable root must never broaden migration eligibility
                warn!(?error, "stock theme migration detection failed closed");
                self.theme_migration = None;
                self.clear_reload_notice(ReloadNoticeKind::ThemeMigration);
            }
        }
    }

    fn configured_theme_paths(&self) -> Result<ThemePaths, ConfigError> {
        let base = Config::config_dir_for_path(&self.config_path)?;
        self.config.resolve_theme_paths_from(&base)
    }

    fn restore_configured_theme(&mut self, paths: ThemePaths) -> CssReloadReport {
        self.css.update_theme(paths, self.config.theme.clone());
        let report = self.css.reload(unixnotis_ui::css::DEFAULT_CSS);
        self.icon_resolver.clear_missing_cache();
        self.theme_preview_active = false;
        report
    }

    fn show_theme_migration_failure(&mut self, migration: &StockThemeMigration, message: &str) {
        self.show_theme_migration_notice(migration, message, true);
    }

    fn show_theme_migration_notice(
        &mut self,
        migration: &StockThemeMigration,
        message: &str,
        error: bool,
    ) {
        self.set_reload_notice(
            ReloadNoticeKind::ThemeMigration,
            message,
            error,
            migration.fingerprint(),
        );
    }
}
