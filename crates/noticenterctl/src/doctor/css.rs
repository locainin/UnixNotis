//! Active theme path and CSS validation checks

use std::path::Path;

use unixnotis_core::Config;

use super::config::redact_home;
use super::model::{DoctorCheck, DoctorSeverity};

pub(super) fn inspect_css(config_path: &Path, config: &Config) -> Vec<DoctorCheck> {
    // Theme paths stay anchored beside the accepted configuration file
    let config_dir = match Config::config_dir_for_path(config_path) {
        Ok(path) => path,
        Err(error) => {
            return vec![DoctorCheck::new(
                "css.theme-paths",
                "Theme paths",
                DoctorSeverity::Error,
                "Theme base directory could not be resolved",
            )
            .details(error.to_string())];
        }
    };
    let theme = match config.resolve_theme_paths_from(&config_dir) {
        Ok(paths) => paths,
        Err(error) => {
            return vec![DoctorCheck::new(
                "css.theme-paths",
                "Theme paths",
                DoctorSeverity::Error,
                "Active theme paths could not be resolved",
            )
            .details(error.to_string())];
        }
    };
    // List every active layer even when a later parser check is incomplete
    let details = [
        ("base", theme.base_css),
        ("panel", theme.panel_css),
        ("popup", theme.popup_css),
        ("widgets", theme.widgets_css),
        ("media", theme.media_css),
    ]
    .into_iter()
    // Each slot name stays stable even when the configured file name changes
    .map(|(slot, path)| format!("{slot}: {}", redact_home(&path)))
    .collect::<Vec<_>>()
    .join("\n");
    let mut checks = vec![DoctorCheck::new(
        "css.theme-paths",
        "Theme paths",
        DoctorSeverity::Pass,
        "Active theme paths resolved",
    )
    .details(details)];

    // Headless sessions cannot run GTK parsing but should still receive all other checks
    if let Err(error) = gtk::init() {
        checks.push(
            DoctorCheck::new(
                "css.validation",
                "CSS validation",
                DoctorSeverity::Warning,
                "CSS validation is incomplete because GTK could not initialize",
            )
            .details(error.to_string())
            .hint("Run doctor inside the graphical desktop session for full CSS checks"),
        );
        return checks;
    }

    // Doctor and css-check share one builder so their findings cannot drift
    match crate::css_check::build_report(config_path, config) {
        // Parser errors are objective because GTK would reject the same active stylesheet
        Ok(report) if report.error_count() > 0 => checks.push(
            DoctorCheck::new(
                "css.validation",
                "CSS validation",
                DoctorSeverity::Error,
                format!("CSS validation found {} error(s)", report.error_count()),
            )
            .details(format!("Warnings: {}", report.warning_count()))
            .hint("Run noticenterctl css-check for file and line details"),
        ),
        Ok(report) if report.warning_count() > 0 => checks.push(
            DoctorCheck::new(
                "css.validation",
                "CSS validation",
                DoctorSeverity::Warning,
                format!("CSS validation found {} warning(s)", report.warning_count()),
            )
            .hint("Run noticenterctl css-check for file and line details"),
        ),
        Ok(_) => checks.push(DoctorCheck::new(
            "css.validation",
            "CSS validation",
            DoctorSeverity::Pass,
            "Active theme CSS passed validation",
        )),
        Err(error) => checks.push(
            // Infrastructure failures are incomplete checks rather than invalid CSS
            DoctorCheck::new(
                "css.validation",
                "CSS validation",
                DoctorSeverity::Warning,
                "CSS validation could not be completed",
            )
            .details(error.to_string())
            .hint("Run noticenterctl css-check for more context"),
        ),
    }
    checks
}

#[cfg(test)]
#[path = "tests/css.rs"]
mod tests;
