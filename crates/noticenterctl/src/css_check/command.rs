use std::path::Path;

use anyhow::{anyhow, Context, Result};
use unixnotis_core::Config;

use super::build_report;
use super::report::render_css_check_report_for_stdout;

/// Run CSS validation against the active `UnixNotis` configuration
///
/// # Errors
///
/// Returns an error when GTK cannot initialize, configuration loading fails, report building
/// fails, or CSS parsing reports an objective error
pub fn run() -> Result<()> {
    // GTK must be ready before providers parse the resolved theme layers
    gtk::init().context("initialize gtk")?;

    // Match the path selected by the daemon and control center
    let config_path = Config::active_config_path().context("resolve config path")?;
    let config = load_config_for_path(&config_path)?;
    let report = build_report(&config_path, &config)?;
    let parse_error_count = report.error_count();

    // Keep the complete report visible even when its final status is an error
    println!("{}", render_css_check_report_for_stdout(&report));

    if parse_error_count > 0 {
        return Err(anyhow!("css-check found {parse_error_count} error(s)"));
    }
    Ok(())
}

fn load_config_for_path(config_path: &Path) -> Result<Config> {
    // An absent default file intentionally selects the built-in configuration
    if config_path.exists() {
        Config::load_from_path(config_path).context("load active config")
    } else {
        Config::load_default().context("load built-in config")
    }
}

#[cfg(test)]
#[path = "tests/command.rs"]
mod tests;
