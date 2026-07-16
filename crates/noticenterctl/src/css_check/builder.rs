//! Shared CSS-check report construction

use anyhow::{anyhow, Context, Result};
use std::path::Path;
use unixnotis_core::Config;

use super::cache::validate_css_parse_files;
use super::files::{display_config_root, format_display_path};
use super::geometry::lint_geometry_css_files;
use super::lint::lint_css_files;
use super::report::{CssCheckCategory, CssCheckDiagnostic, CssCheckReport};
use super::runtime::lint_runtime_config;
use super::theme::collect_css_check_inputs_from;

pub fn build_report(config_path: &Path, config: &Config) -> Result<CssCheckReport> {
    // The supplied config path controls relative theme and asset resolution
    let config_dir =
        Config::config_dir_for_path(config_path).context("resolve config directory")?;
    let display_root = display_config_root(&config_dir);
    if !config_dir.exists() {
        return Err(anyhow!("config directory not found: {display_root}"));
    }
    if !config_dir.is_dir() {
        return Err(anyhow!("config path is not a directory: {display_root}"));
    }

    // Follow the same accepted config and path anchor used by the caller
    let css_inputs =
        collect_css_check_inputs_from(&config_dir, &display_root, config_path, config)?;
    let css_files = css_inputs.files;
    if css_files.is_empty() {
        return Err(anyhow!("no active css files found for {display_root}"));
    }

    let mut diagnostics = css_inputs.diagnostics;
    let mut parse_candidates = Vec::new();
    for path in &css_files {
        // Bad paths should show up before GTK tries to parse them
        if !path.exists() {
            let display_path = format_display_path(&config_dir, &display_root, path);
            diagnostics.push(CssCheckDiagnostic::error(
                CssCheckCategory::Parse,
                display_path,
                None,
                None,
                "file not found",
                None,
            ));
            continue;
        }
        if !path.is_file() {
            // Directories and special files never reach GTK or text linters
            let display_path = format_display_path(&config_dir, &display_root, path);
            diagnostics.push(CssCheckDiagnostic::error(
                CssCheckCategory::Parse,
                display_path,
                None,
                None,
                "not a regular file",
                None,
            ));
            continue;
        }
        // Real files move through the cache-aware GTK parse path next
        parse_candidates.push(path.clone());
    }

    let parse_report = validate_css_parse_files(&parse_candidates, &config_dir, &display_root)?;
    // Parser count gives the diagnostics vector an exact lower-bound growth hint
    diagnostics.reserve(parse_report.error_count);
    diagnostics.extend(parse_report.diagnostics);
    diagnostics.extend(lint_css_files(&css_files, &config_dir, &display_root)?);
    // Runtime checks compare configured geometry with the same active CSS set
    diagnostics.extend(lint_runtime_config(
        &config_dir,
        &display_root,
        config_path,
        config,
    )?);
    diagnostics.extend(lint_geometry_css_files(
        &css_files,
        &config_dir,
        &display_root,
    )?);

    Ok(CssCheckReport {
        // Counts describe requested active layers even when one file was invalid
        display_root,
        checked_files: css_files.len(),
        active_files: css_inputs.active_files,
        notes: css_inputs.notes,
        diagnostics,
    })
}

#[cfg(test)]
#[path = "tests/builder.rs"]
mod tests;
