//! Report model and rendering for css-check

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum CssCheckSeverity {
    Error,
    Warning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum CssCheckCategory {
    Parse,
    Theme,
    Runtime,
    Lint,
    Geometry,
}

impl CssCheckCategory {
    fn label(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Theme => "theme",
            Self::Runtime => "runtime",
            Self::Lint => "lint",
            Self::Geometry => "geometry",
        }
    }

    fn summary_label(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Theme => "theme",
            Self::Runtime => "runtime",
            Self::Lint => "lint",
            Self::Geometry => "geometry",
        }
    }

    fn sort_rank(self) -> u8 {
        match self {
            Self::Parse => 0,
            Self::Theme => 1,
            Self::Runtime => 2,
            Self::Lint => 3,
            Self::Geometry => 4,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CssCheckDiagnostic {
    pub(super) severity: CssCheckSeverity,
    pub(super) category: CssCheckCategory,
    pub(super) code: &'static str,
    pub(super) display_path: String,
    pub(super) line: Option<usize>,
    pub(super) column: Option<usize>,
    pub(super) message: String,
    pub(super) hint: Option<String>,
}

impl CssCheckDiagnostic {
    pub(super) fn warning(
        category: CssCheckCategory,
        code: &'static str,
        display_path: String,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity: CssCheckSeverity::Warning,
            category,
            code,
            display_path,
            line: None,
            column: None,
            message: message.into(),
            hint: None,
        }
    }

    pub(super) fn error(
        category: CssCheckCategory,
        code: &'static str,
        display_path: String,
        line: Option<usize>,
        column: Option<usize>,
        message: impl Into<String>,
        hint: Option<String>,
    ) -> Self {
        Self {
            severity: CssCheckSeverity::Error,
            category,
            code,
            display_path,
            line,
            column,
            message: message.into(),
            hint,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CssCheckActiveFile {
    pub(super) slot_name: &'static str,
    pub(super) display_path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct CssCheckReport {
    pub(super) display_root: String,
    pub(super) checked_files: usize,
    pub(super) active_files: Vec<CssCheckActiveFile>,
    pub(super) notes: Vec<String>,
    pub(super) diagnostics: Vec<CssCheckDiagnostic>,
}

impl CssCheckReport {
    pub(super) fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|item| item.severity == CssCheckSeverity::Error)
            .count()
    }

    pub(super) fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|item| item.severity == CssCheckSeverity::Warning)
            .count()
    }

    pub(super) fn is_clean(&self) -> bool {
        self.diagnostics.is_empty() && self.notes.is_empty()
    }
}

pub(super) fn render_css_check_report(report: &CssCheckReport) -> String {
    let mut lines = Vec::new();
    let errors = report.error_count();
    let warnings = report.warning_count();

    lines.push("css-check summary".to_string());
    lines.push(format!("  root: {}", report.display_root));
    lines.push(format!("  checked: {} files", report.checked_files));
    lines.push(format!("  errors: {errors}"));
    lines.push(format!("  warnings: {warnings}"));

    if !report.diagnostics.is_empty() {
        // Show the issue mix first so large reports are easier to scan
        let category_counts = diagnostic_category_counts(&report.diagnostics);
        if !category_counts.is_empty() {
            lines.push(String::new());
            lines.push("diagnostic categories".to_string());
            for (label, count) in category_counts {
                lines.push(format!("  {label}: {count}"));
            }
        }

        let top_files = top_problem_files(&report.diagnostics);
        if report.diagnostics.len() > 3 && !top_files.is_empty() {
            // Large reports are easier to read when the noisiest files are called out early
            lines.push(String::new());
            lines.push("top problem files".to_string());
            for (display_path, count) in top_files {
                lines.push(format!("  {display_path}: {count} issue(s)"));
            }
        }
    }

    if !report.active_files.is_empty() && !report.is_clean() {
        // Active theme files matter most once something needs attention
        lines.push(String::new());
        lines.push("active theme files".to_string());
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
                path = active_file.display_path,
                width = slot_width
            ));
        }
    }

    if !report.notes.is_empty() {
        // Notes hold extra context that should not drown out real diagnostics
        lines.push(String::new());
        lines.push("notes".to_string());
        for note in &report.notes {
            lines.push(format!("  {note}"));
        }
    }

    append_diagnostic_section(
        &mut lines,
        "errors",
        &collect_grouped_diagnostics(&report.diagnostics, CssCheckSeverity::Error),
    );
    append_diagnostic_section(
        &mut lines,
        "warnings",
        &collect_grouped_diagnostics(&report.diagnostics, CssCheckSeverity::Warning),
    );

    lines.push(String::new());
    lines.push(format!(
        "css-check result: {}",
        if errors > 0 {
            "failed"
        } else if warnings > 0 {
            "warnings found"
        } else {
            "clean"
        }
    ));

    lines.join("\n")
}

fn diagnostic_category_counts(diagnostics: &[CssCheckDiagnostic]) -> Vec<(&'static str, usize)> {
    let mut counts = [0usize; 5];
    for diagnostic in diagnostics {
        counts[diagnostic.category.sort_rank() as usize] += 1;
    }

    let mut result = Vec::new();
    for category in [
        CssCheckCategory::Parse,
        CssCheckCategory::Theme,
        CssCheckCategory::Runtime,
        CssCheckCategory::Lint,
        CssCheckCategory::Geometry,
    ] {
        let count = counts[category.sort_rank() as usize];
        if count > 0 {
            result.push((category.summary_label(), count));
        }
    }
    result
}

fn top_problem_files(diagnostics: &[CssCheckDiagnostic]) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for diagnostic in diagnostics {
        *counts.entry(diagnostic.display_path.clone()).or_insert(0) += 1;
    }

    let mut items = counts.into_iter().collect::<Vec<_>>();
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
    grouped.into_iter().collect()
}

fn append_diagnostic_section(
    lines: &mut Vec<String>,
    title: &str,
    grouped: &[(String, Vec<CssCheckDiagnostic>)],
) {
    if grouped.is_empty() {
        return;
    }

    lines.push(String::new());
    lines.push(title.to_string());
    for (display_path, diagnostics) in grouped {
        // File counts help large reports read faster
        lines.push(String::new());
        lines.push(format!("  {display_path} ({})", diagnostics.len()));
        for diagnostic in diagnostics {
            let location = match (diagnostic.line, diagnostic.column) {
                (Some(line), Some(column)) => format!(" line {line}, col {column}:"),
                (Some(line), None) => format!(" line {line}:"),
                _ => String::new(),
            };
            lines.push(format!(
                "    [{}][{}]{} {}",
                diagnostic.code,
                diagnostic.category.label(),
                location,
                diagnostic.message
            ));
            if let Some(hint) = diagnostic.hint.as_ref() {
                lines.push(format!("      hint: {hint}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        render_css_check_report, CssCheckActiveFile, CssCheckCategory, CssCheckDiagnostic,
        CssCheckReport,
    };

    #[test]
    fn render_report_groups_warnings_by_file_and_category() {
        let report = CssCheckReport {
            display_root: "$HOME/.config/unixnotis".to_string(),
            checked_files: 5,
            active_files: vec![
                CssCheckActiveFile {
                    slot_name: "[theme].base_css",
                    display_path: "$HOME/.config/unixnotis/base.css".to_string(),
                },
                CssCheckActiveFile {
                    slot_name: "[theme].widgets_css",
                    display_path: "$HOME/.config/unixnotis/widgets.css".to_string(),
                },
            ],
            notes: vec!["2 extra css file(s) under $HOME/.config/unixnotis were skipped because config.toml does not reference them".to_string()],
            diagnostics: vec![
                CssCheckDiagnostic::warning(
                    CssCheckCategory::Lint,
                    "LINT002",
                    "$HOME/.config/unixnotis/widgets.css".to_string(),
                    "duplicate selector '.unixnotis-info-icon'",
                ),
                CssCheckDiagnostic::warning(
                    CssCheckCategory::Theme,
                    "THEME007",
                    "$HOME/.config/unixnotis/base.css".to_string(),
                    "css asset reference points outside $HOME/.config/unixnotis",
                ),
            ],
        };

        let rendered = render_css_check_report(&report);
        assert!(rendered.contains("css-check summary"));
        assert!(rendered.contains("diagnostic categories"));
        assert!(rendered.contains("active theme files"));
        assert!(rendered.contains("notes"));
        assert!(rendered.contains("[LINT002][lint] duplicate selector '.unixnotis-info-icon'"));
        assert!(rendered.contains("[THEME007][theme] css asset reference points outside"));
        assert!(rendered.contains("css-check result: warnings found"));
    }

    #[test]
    fn render_report_keeps_clean_output_short() {
        let report = CssCheckReport {
            display_root: "$HOME/.config/unixnotis".to_string(),
            checked_files: 5,
            ..CssCheckReport::default()
        };

        let rendered = render_css_check_report(&report);
        assert!(rendered.contains("checked: 5 files"));
        assert!(rendered.contains("css-check result: clean"));
        assert!(!rendered.contains("active theme files"));
        assert!(!rendered.contains("\nwarnings\n"));
    }
}
