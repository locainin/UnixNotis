//! CSS validation and lint helpers for UnixNotis themes

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
use gtk::prelude::*;
use gtk::CssProvider;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use unixnotis_core::Config;

use self::main_css_check_files::{display_config_root, format_display_path};
use self::main_css_check_geometry::lint_geometry_css_files;
use self::main_css_check_lint::lint_css_files;
use self::main_css_check_policy::parsing_error_hint;
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

    // Collect parser failures so the final report stays grouped
    let error_count = Arc::new(AtomicUsize::new(0));
    let parse_errors = Arc::new(Mutex::new(Vec::<CssCheckDiagnostic>::new()));
    let provider = CssProvider::new();
    let error_count_clone = Arc::clone(&error_count);
    let parse_errors_clone = Arc::clone(&parse_errors);
    let config_root = config_dir.clone();
    let display_root_clone = display_root.clone();
    provider.connect_parsing_error(move |_provider, section, error| {
        error_count_clone.fetch_add(1, Ordering::Relaxed);
        let location = section.start_location();
        // Keep GTK paths in the same display style as the rest of css-check
        let file = section
            .file()
            .and_then(|file| file.path())
            .map(|path| format_display_path(&config_root, &display_root_clone, &path))
            .unwrap_or_else(|| "<data>".to_string());
        // A small hint from the broken line makes GTK parser errors easier to act on
        let hint = source_line_text(
            section.file().and_then(|file| file.path()).as_deref(),
            location.lines() + 1,
        )
        .and_then(|line_text| parsing_error_hint(&line_text));
        let mut diagnostics = parse_errors_clone.lock().expect("parse error lock");
        diagnostics.push(CssCheckDiagnostic::error(
            CssCheckCategory::Parse,
            file,
            Some(location.lines() + 1),
            Some(location.line_chars() + 1),
            error.message(),
            hint,
        ));
    });

    let mut diagnostics = css_inputs.diagnostics;
    for path in &css_files {
        // Bad paths should show up before GTK tries to parse them
        if !path.exists() {
            error_count.fetch_add(1, Ordering::Relaxed);
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
            error_count.fetch_add(1, Ordering::Relaxed);
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
        // Feed every file into one provider so parsing matches the live app
        provider.load_from_path(path);
    }

    diagnostics.extend(
        parse_errors
            .lock()
            .expect("parse error mutex poisoned")
            .clone(),
    );
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

    let errors = error_count.load(Ordering::Relaxed);
    if errors > 0 {
        return Err(anyhow!("css-check found {errors} error(s)"));
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
