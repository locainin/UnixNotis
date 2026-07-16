//! Shared CSS-check report construction

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use unixnotis_core::Config;

use super::cache::validate_css_parse_files;
use super::files::{display_config_root, format_display_path};
use super::geometry::lint_geometry_css_files_with_config;
use super::lint::lint_css_files;
use super::report::{CssCheckCategory, CssCheckDiagnostic, CssCheckReport};
use super::runtime::{display_config_path, lint_runtime_config};
use super::theme::collect_css_check_inputs_from;

pub fn build_report(config_path: &Path, config: &Config) -> Result<CssCheckReport> {
    // Production parsing is injected below so report aggregation stays directly testable
    build_report_with_parser(config_path, config, |files, config_dir, display_root| {
        let report = validate_css_parse_files(files, config_dir, display_root)?;
        Ok((report.diagnostics, report.error_count))
    })
}

fn build_report_with_parser(
    config_path: &Path,
    config: &Config,
    parse_files: impl FnOnce(&[PathBuf], &Path, &str) -> Result<(Vec<CssCheckDiagnostic>, usize)>,
) -> Result<CssCheckReport> {
    // The supplied config path controls relative theme and asset resolution
    let config_dir =
        Config::config_dir_for_path(config_path).context("resolve config directory")?;
    let display_root = display_config_root(&config_dir);
    if !config_dir.exists() {
        // A missing root cannot produce meaningful relative theme diagnostics
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
        // Accepted configs still require at least one active stylesheet to validate
        return Err(anyhow!("no active css files found for {display_root}"));
    }

    let mut diagnostics = css_inputs.diagnostics;
    // Readable files are collected separately so one bad layer does not stop later checks
    let mut readable_files = Vec::new();
    let theme_paths = config
        .resolve_theme_paths_from(&config_dir)
        .context("resolve theme paths for css-check")?;
    for path in [
        theme_paths.base_css,
        theme_paths.panel_css,
        theme_paths.popup_css,
        theme_paths.widgets_css,
        theme_paths.media_css,
    ] {
        // Each configured theme slot receives an explicit missing or wrong-type diagnostic
        if !path.exists() {
            diagnostics.push(CssCheckDiagnostic::error(
                CssCheckCategory::Parse,
                format_display_path(&config_dir, &display_root, &path),
                None,
                None,
                "file not found",
                None,
            ));
        } else if !path.is_file() {
            diagnostics.push(CssCheckDiagnostic::error(
                CssCheckCategory::Parse,
                format_display_path(&config_dir, &display_root, &path),
                None,
                None,
                "not a regular file",
                None,
            ));
        }
    }
    for path in &css_files {
        // A file can disappear after theme collection, so recheck before every reader
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
        // Only verified regular files may reach readers that otherwise abort the report
        readable_files.push(path.clone());
    }

    let (parse_diagnostics, parse_error_count) =
        parse_files(&readable_files, &config_dir, &display_root)?;
    // Parser count gives the diagnostics vector an exact lower-bound growth hint
    diagnostics.reserve(parse_error_count);
    diagnostics.extend(parse_diagnostics);
    diagnostics.extend(lint_css_files(
        &readable_files,
        &config_dir,
        &display_root,
        config,
    )?);
    // Text linting runs even when GTK parsing reported an independent layer error
    // Runtime checks compare configured geometry with the same active CSS set
    diagnostics.extend(lint_runtime_config(
        &config_dir,
        &display_root,
        config_path,
        config,
    )?);
    let config_display = display_config_path(&config_dir, &display_root, config_path);
    // Geometry checks use the same filtered files and resolved config path as earlier stages
    diagnostics.extend(lint_geometry_css_files_with_config(
        &readable_files,
        &config_dir,
        &display_root,
        &config_display,
        config,
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
