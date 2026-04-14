use std::collections::BTreeMap;

use super::model::{CssCheckDiagnostic, CssCheckReport, CssCheckSeverity};
use super::style::ReportStyle;

pub(in super::super) fn render_css_check_report_for_stdout(report: &CssCheckReport) -> String {
    // This path picks color at the last moment so the report model stays plain
    render_css_check_report_with_style(report, ReportStyle::for_stdout())
}

pub(in super::super) fn render_css_check_report_with_style(
    report: &CssCheckReport,
    style: ReportStyle,
) -> String {
    let mut lines = Vec::new();
    let errors = report.error_count();
    let warnings = report.warning_count();

    // The summary stays first no matter how noisy the report gets
    lines.push(style.bold("css-check summary"));
    lines.push(format!("  root: {}", report.display_root));
    lines.push(format!("  checked: {} files", report.checked_files));
    lines.push(format!("  errors: {errors}"));
    lines.push(format!("  warnings: {warnings}"));

    if !report.diagnostics.is_empty() {
        // Show the issue mix first so large reports are easier to scan
        let category_counts = diagnostic_category_counts(&report.diagnostics);
        if !category_counts.is_empty() {
            lines.push(String::new());
            lines.push(style.categories_title("diagnostic categories"));
            for (label, count) in category_counts {
                lines.push(format!("  {label}: {count}"));
            }
        }

        let top_files = top_problem_files(&report.diagnostics);
        if report.diagnostics.len() > 3 && !top_files.is_empty() {
            // Large reports are easier to read when the noisiest files are called out early
            lines.push(String::new());
            lines.push(style.bold("top problem files"));
            for (display_path, count) in top_files {
                lines.push(format!("  {}: {count} issue(s)", style.bold(display_path)));
            }
        }
    }

    if !report.active_files.is_empty() && !report.is_clean() {
        // Active theme files matter most once something needs attention
        lines.push(String::new());
        lines.push(style.bold("active theme files"));
        let slot_width = report
            .active_files
            .iter()
            .map(|file| file.slot_name.len())
            .max()
            .unwrap_or(0);
        for active_file in &report.active_files {
            lines.push(format!(
                "  {slot:<width$} -> {path}",
                slot = active_file.slot_name,
                path = style.bold(&active_file.display_path),
                width = slot_width
            ));
        }
    }

    if !report.notes.is_empty() {
        // Notes hold extra context that should not drown out real diagnostics
        lines.push(String::new());
        lines.push(style.notes_title("notes"));
        for note in &report.notes {
            lines.push(format!("  {note}"));
        }
    }

    append_diagnostic_section(
        &mut lines,
        style.errors_title("errors"),
        &collect_grouped_diagnostics(&report.diagnostics, CssCheckSeverity::Error),
        style,
    );
    append_diagnostic_section(
        &mut lines,
        style.warnings_title("warnings"),
        &collect_grouped_diagnostics(&report.diagnostics, CssCheckSeverity::Warning),
        style,
    );

    lines.push(String::new());
    // One final verdict line makes shell use and quick scans easier
    let result_text = if errors > 0 {
        "failed"
    } else if warnings > 0 {
        "warnings found"
    } else {
        "clean"
    };
    lines.push(format!(
        "css-check result: {}",
        style.result(result_text, errors, warnings)
    ));

    lines.join("\n")
}

fn diagnostic_category_counts(diagnostics: &[CssCheckDiagnostic]) -> Vec<(&'static str, usize)> {
    // Fixed slots keep the category summary stable from run to run
    let mut counts = [0usize; 5];
    for diagnostic in diagnostics {
        counts[diagnostic.category.sort_rank() as usize] += 1;
    }

    let mut result = Vec::new();
    for category in [
        super::model::CssCheckCategory::Parse,
        super::model::CssCheckCategory::Theme,
        super::model::CssCheckCategory::Runtime,
        super::model::CssCheckCategory::Lint,
        super::model::CssCheckCategory::Geometry,
    ] {
        // Empty categories stay hidden so small reports stay short
        let count = counts[category.sort_rank() as usize];
        if count > 0 {
            result.push((category.summary_label(), count));
        }
    }
    result
}

fn top_problem_files(diagnostics: &[CssCheckDiagnostic]) -> Vec<(String, usize)> {
    // File totals make big reports easier to skim before reading each section
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for diagnostic in diagnostics {
        *counts.entry(diagnostic.display_path.clone()).or_insert(0) += 1;
    }

    let mut items = counts.into_iter().collect::<Vec<_>>();
    // Most noisy files should rise to the top, with path order breaking ties
    items.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    items.into_iter().take(5).collect()
}

fn collect_grouped_diagnostics(
    diagnostics: &[CssCheckDiagnostic],
    severity: CssCheckSeverity,
) -> Vec<(String, Vec<CssCheckDiagnostic>)> {
    // Stable ordering keeps test output and terminal output predictable
    let mut items = diagnostics
        .iter()
        .filter(|item| item.severity == severity)
        .cloned()
        .collect::<Vec<_>>();
    // This keeps parse before theme, then runtime, lint, and geometry
    items.sort_by(|left, right| {
        left.category
            .sort_rank()
            .cmp(&right.category.sort_rank())
            .then_with(|| left.display_path.cmp(&right.display_path))
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.column.cmp(&right.column))
            .then_with(|| left.code.cmp(right.code))
            .then_with(|| left.message.cmp(&right.message))
    });

    let mut grouped = BTreeMap::<String, Vec<CssCheckDiagnostic>>::new();
    for item in items {
        // Group by file so repeated warnings read like one report instead of a log stream
        grouped
            .entry(item.display_path.clone())
            .or_default()
            .push(item);
    }
    // BTreeMap keeps file order deterministic without an extra sort pass here
    grouped.into_iter().collect()
}

fn append_diagnostic_section(
    lines: &mut Vec<String>,
    title: String,
    grouped: &[(String, Vec<CssCheckDiagnostic>)],
    style: ReportStyle,
) {
    if grouped.is_empty() {
        return;
    }

    lines.push(String::new());
    lines.push(title);
    for (display_path, diagnostics) in grouped {
        // File counts help large reports read faster
        lines.push(String::new());
        lines.push(format!(
            "  {} ({})",
            style.bold(display_path),
            diagnostics.len()
        ));
        for diagnostic in diagnostics {
            // Line and column only show when the source path has a real parser location
            let location = match (diagnostic.line, diagnostic.column) {
                (Some(line), Some(column)) => format!(" line {line}, col {column}:"),
                (Some(line), None) => format!(" line {line}:"),
                _ => String::new(),
            };
            lines.push(format!(
                "    {}{} {}",
                style.diagnostic_code(format!(
                    "[{}][{}]",
                    diagnostic.code,
                    diagnostic.category.label()
                )),
                location,
                diagnostic.message
            ));
            if let Some(hint) = diagnostic.hint.as_ref() {
                // Hints stay on their own line so the main warning text stays short
                lines.push(format!("      hint: {hint}"));
            }
        }
    }
}
