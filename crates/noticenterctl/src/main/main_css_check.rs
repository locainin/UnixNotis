//! CSS validation and lint helpers for UnixNotis themes

#[path = "main_css_check_cache.rs"]
mod main_css_check_cache;
#[path = "main_css_check_files.rs"]
mod main_css_check_files;
#[path = "main_css_check_geometry.rs"]
mod main_css_check_geometry;
#[path = "main_css_check_lint/mod.rs"]
mod main_css_check_lint;
#[path = "main_css_check_parse.rs"]
mod main_css_check_parse;
#[path = "main_css_check_policy.rs"]
mod main_css_check_policy;
#[path = "main_css_check_report/mod.rs"]
mod main_css_check_report;
#[path = "main_css_check_runtime.rs"]
mod main_css_check_runtime;
#[path = "main_css_check_theme.rs"]
mod main_css_check_theme;

use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::Path;
use unixnotis_core::Config;

use self::main_css_check_cache::validate_css_parse_files;
use self::main_css_check_files::{display_config_root, format_display_path};
use self::main_css_check_geometry::lint_geometry_css_files;
use self::main_css_check_lint::lint_css_files;
use self::main_css_check_report::{
    render_css_check_report_for_stdout, CssCheckCategory, CssCheckDiagnostic, CssCheckReport,
};
use self::main_css_check_runtime::lint_runtime_config;
use self::main_css_check_theme::collect_css_check_inputs;

pub(crate) fn run_css_check() -> Result<()> {
    // GTK needs to be ready before CSS parsing starts
    gtk::init().context("initialize gtk")?;

    // css-check always reads the live config tree
    let config_dir = Config::default_config_dir().context("resolve config directory")?;
    let display_root = display_config_root(&config_dir);
    if !config_dir.exists() {
        return Err(anyhow!("config directory not found: {}", display_root));
    }
    if !config_dir.is_dir() {
        return Err(anyhow!("config path is not a directory: {}", display_root));
    }

    // Follow the same theme targets the live UI resolves from config.toml
    let css_inputs = collect_css_check_inputs(&config_dir, &display_root)?;
    let css_files = css_inputs.files;
    if css_files.is_empty() {
        return Err(anyhow!("no active css files found for {}", display_root));
    }

    let mut diagnostics = css_inputs.diagnostics;
    let mut parse_candidates = Vec::new();
    let mut parse_error_count = 0usize;
    for path in &css_files {
        // Bad paths should show up before GTK tries to parse them
        if !path.exists() {
            parse_error_count += 1;
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
            parse_error_count += 1;
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
    parse_error_count += parse_report.error_count;
    diagnostics.extend(parse_report.diagnostics);
    diagnostics.extend(lint_css_files(&css_files, &config_dir, &display_root)?);
    diagnostics.extend(lint_runtime_config(&config_dir, &display_root)?);
    diagnostics.extend(lint_geometry_css_files(
        &css_files,
        &config_dir,
        &display_root,
    )?);

    let report = CssCheckReport {
        display_root: display_root.clone(),
        checked_files: css_files.len(),
        active_files: css_inputs.active_files,
        notes: css_inputs.notes,
        diagnostics,
    };
    println!("{}", render_css_check_report_for_stdout(&report));

    if parse_error_count > 0 {
        return Err(anyhow!("css-check found {parse_error_count} error(s)"));
    }
    Ok(())
}

fn source_line_text(path: Option<&Path>, line_number: usize) -> Option<String> {
    let path = path?;
    if line_number == 0 {
        return None;
    }
    // Read only when a parser error needs a hint
    let contents = fs::read_to_string(path).ok()?;
    contents
        .lines()
        // GTK line numbers start at one
        .nth(line_number.saturating_sub(1))
        .map(str::to_string)
}
#[cfg(test)]
#[path = "main_css_check_tests.rs"]
mod tests;
