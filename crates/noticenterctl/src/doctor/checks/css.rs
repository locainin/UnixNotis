//! Active theme path and CSS validation checks

use std::path::Path;

use unixnotis_core::Config;

use super::super::report::{redact_home, safe_doctor_text};
use super::super::report::{DoctorCheck, DoctorSeverity};

pub(in crate::doctor) fn inspect_css(config_path: &Path, config: &Config) -> Vec<DoctorCheck> {
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
            .details(safe_doctor_text(&error.to_string()))];
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
            .details(safe_doctor_text(&error.to_string()))];
        }
    };
    // List every active layer even when a later parser check is incomplete
    let details = [
        ("base", &theme.base_css),
        ("panel", &theme.panel_css),
        ("popup", &theme.popup_css),
        ("widgets", &theme.widgets_css),
        ("media", &theme.media_css),
    ]
    .into_iter()
    // Each slot name stays stable even when the configured file name changes
    .map(|(slot, path)| format!("{slot}: {}", redact_home(path)))
    .collect::<Vec<_>>()
    .join("\n");
    let mut checks = vec![DoctorCheck::new(
        "css.theme-paths",
        "Theme paths",
        DoctorSeverity::Pass,
        "Active theme paths resolved",
    )
    .details(details)
    .data("base", redact_home(&theme.base_css))
    .data("panel", redact_home(&theme.panel_css))
    .data("popup", redact_home(&theme.popup_css))
    .data("widgets", redact_home(&theme.widgets_css))
    .data("media", redact_home(&theme.media_css))];

    // Headless sessions cannot run GTK parsing but should still receive all other checks
    if let Err(error) = gtk::init() {
        checks.push(
            DoctorCheck::new(
                "css.validation",
                "CSS validation",
                DoctorSeverity::Warning,
                "CSS validation is incomplete because GTK could not initialize",
            )
            .details(safe_doctor_text(&error.to_string()))
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
            .details(safe_doctor_text(&error.to_string()))
            .hint("Run noticenterctl css-check for more context"),
        ),
    }
    checks
}
