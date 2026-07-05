use anyhow::Result;
use gtk::prelude::*;
use gtk::CssProvider;
use std::fs;
use std::path::Path;

use super::super::main_css_check_files::format_display_path;
use super::super::main_css_check_policy::parsing_error_hint;
use super::super::main_css_check_report::{CssCheckCategory, CssCheckDiagnostic};
use super::super::source_line_text;
use super::model::{CachedDiagnosticSource, CachedParseDiagnostic, CssParseWorkItem};

pub(in super::super) fn parse_css_file_with_gtk(
    work_item: &CssParseWorkItem,
) -> Result<Vec<CachedParseDiagnostic>> {
    // One provider per file keeps parser state isolated
    let provider = CssProvider::new();
    let current_file = work_item.canonical_path.clone();
    let findings = std::rc::Rc::new(std::cell::RefCell::new(Vec::<CachedParseDiagnostic>::new()));
    let findings_for_signal = findings.clone();

    provider.connect_parsing_error(move |_provider, section, error| {
        let location = section.start_location();
        let line = location.lines() + 1;
        let source_path = section.file().and_then(|file| file.path());

        // Line hints stay tied to the exact file GTK blamed
        let hint = source_line_text(source_path.as_deref(), line)
            .and_then(|line_text| parsing_error_hint(&line_text));

        let source = classify_cached_source_path(source_path.as_deref(), &current_file);
        findings_for_signal
            .borrow_mut()
            .push(CachedParseDiagnostic {
                source,
                line: Some(line),
                column: Some(location.line_chars() + 1),
                message: error.message().to_string(),
                hint,
            });
    });

    // Gtk clears prior provider state on every load_from_path call
    provider.load_from_path(&work_item.load_path);
    let diagnostics = findings.borrow().clone();
    Ok(diagnostics)
}

pub(in super::super) fn render_cached_diagnostics(
    diagnostics: &[CachedParseDiagnostic],
    work_item: &CssParseWorkItem,
    config_dir: &Path,
    display_root: &str,
) -> Vec<CssCheckDiagnostic> {
    // Top-level errors should always point at the current logical input path
    let top_level_display = format_display_path(config_dir, display_root, &work_item.load_path);
    diagnostics
        .iter()
        .map(|diagnostic| {
            let display_path = match &diagnostic.source {
                CachedDiagnosticSource::TopLevel => top_level_display.clone(),
                CachedDiagnosticSource::Path(path) => {
                    format_display_path(config_dir, display_root, path)
                }
                CachedDiagnosticSource::Data => "<data>".to_string(),
            };

            CssCheckDiagnostic::error(
                CssCheckCategory::Parse,
                display_path,
                diagnostic.line,
                diagnostic.column,
                diagnostic.message.clone(),
                diagnostic.hint.clone(),
            )
        })
        .collect()
}

fn classify_cached_source_path(
    source_path: Option<&Path>,
    current_file: &Path,
) -> CachedDiagnosticSource {
    // Missing source info still needs a stable bucket in the cached form
    let Some(source_path) = source_path else {
        return CachedDiagnosticSource::Data;
    };

    // Imported files should only be treated as top-level when they resolve back to the same file
    let normalized_source =
        fs::canonicalize(source_path).unwrap_or_else(|_| source_path.to_path_buf());
    if normalized_source == current_file {
        return CachedDiagnosticSource::TopLevel;
    }

    CachedDiagnosticSource::Path(source_path.to_path_buf())
}
