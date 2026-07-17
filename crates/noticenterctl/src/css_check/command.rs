use std::path::Path;

use anyhow::{anyhow, Context, Result};
use unixnotis_core::Config;

use crate::config_path::{resolve_config_path, ConfigPathSource};

use super::build_report;
use super::report::render_css_check_report_for_stdout;

/// Run CSS validation against the active `UnixNotis` configuration
///
/// # Errors
///
/// Returns an error when configuration loading fails, report building fails, or CSS parsing
/// reports an objective error
pub fn run(requested_path: Option<std::path::PathBuf>) -> Result<()> {
    // An explicit command path must outrank the environment and normal default location
    let (config_path, source) =
        resolve_config_path(requested_path).context("resolve config path")?;
    let config = load_config_for_path(&config_path, source)?;
    let report = build_report(&config_path, &config)?;
    let parse_error_count = report.error_count();

    // Keep the complete report visible even when its final status is an error
    println!("{}", render_css_check_report_for_stdout(&report));

    if parse_error_count > 0 {
        return Err(anyhow!("css-check found {parse_error_count} error(s)"));
    }
    Ok(())
}

fn load_config_for_path(config_path: &Path, source: ConfigPathSource) -> Result<Config> {
    match std::fs::symlink_metadata(config_path) {
        // Existing malformed or unsafe targets must fail without a fallback
        Ok(_) => Config::load_from_path(config_path).context("load active config"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && source.is_explicit() => {
            Err(anyhow!(
                "explicit configuration file does not exist: {}",
                config_path.display()
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // Only the normal absent default path may select embedded defaults
            Config::load_default().context("load built-in config")
        }
        Err(error) => Err(error)
            .with_context(|| format!("inspect configuration path {}", config_path.display())),
    }
}

#[cfg(test)]
#[path = "tests/command.rs"]
mod tests;
