//! Report model and rendering for css-check

use std::collections::BTreeMap;
use std::env;
use std::io::{self, IsTerminal};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReportStyle {
    color: bool,
}

impl ReportStyle {
    fn for_stdout() -> Self {
        // Color should stay off for pipes, NO_COLOR, or dumb terminals
        let color = io::stdout().is_terminal()
            && env::var_os("NO_COLOR").is_none()
            && env::var("CLICOLOR")
                .map(|value| value != "0")
                .unwrap_or(true)
            && env::var("TERM")
                .map(|value| value != "dumb")
                .unwrap_or(true);
        Self { color }
    }

    fn paint(self, text: impl Into<String>, prefix: &str) -> String {
        let text = text.into();
        if !self.color {
            // Plain output keeps tests and redirected output stable
            return text;
        }

        // One reset at the end keeps nested formatting simple
        format!("\u{1b}[{prefix}m{text}\u{1b}[0m")
    }

    fn bold(self, text: impl Into<String>) -> String {
        self.paint(text, "1")
    }

    fn errors_title(self, text: impl Into<String>) -> String {
        self.paint(text, "1;31")
    }

    fn warnings_title(self, text: impl Into<String>) -> String {
        self.paint(text, "1;33")
    }

    fn notes_title(self, text: impl Into<String>) -> String {
        self.paint(text, "1;36")
    }

    fn categories_title(self, text: impl Into<String>) -> String {
        self.paint(text, "1;35")
    }

    fn result(self, text: impl Into<String>, errors: usize, warnings: usize) -> String {
        if errors > 0 {
            return self.paint(text, "1;31");
        }
        if warnings > 0 {
            return self.paint(text, "1;33");
        }
        self.paint(text, "1;32")
    }

    fn diagnostic_code(self, text: impl Into<String>) -> String {
        self.paint(text, "2;35")
    }
}

pub(super) fn render_css_check_report_for_stdout(report: &CssCheckReport) -> String {
    // This path picks color at the last moment so the report model stays plain
    render_css_check_report_with_style(report, ReportStyle::for_stdout())
}

fn render_css_check_report_with_style(report: &CssCheckReport, style: ReportStyle) -> String {
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

#[cfg(test)]
mod tests {
    use super::{
        render_css_check_report_with_style, CssCheckActiveFile, CssCheckCategory,
        CssCheckDiagnostic, CssCheckReport, ReportStyle,
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

        let rendered = render_css_check_report_with_style(&report, ReportStyle { color: false });
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

        let rendered = render_css_check_report_with_style(&report, ReportStyle { color: false });
        assert!(rendered.contains("checked: 5 files"));
        assert!(rendered.contains("css-check result: clean"));
        assert!(!rendered.contains("active theme files"));
        assert!(!rendered.contains("\nwarnings\n"));
    }

    #[test]
    fn render_report_orders_sections_and_shows_top_problem_files() {
        // Mixed reports should keep the big sections in a stable order
        let report = CssCheckReport {
            display_root: "$HOME/.config/unixnotis".to_string(),
            checked_files: 5,
            active_files: vec![CssCheckActiveFile {
                slot_name: "[theme].base_css",
                display_path: "$HOME/.config/unixnotis/base.css".to_string(),
            }],
            notes: vec![
                "1 configured command path(s) point outside $HOME/.config/unixnotis".to_string(),
            ],
            diagnostics: vec![
                CssCheckDiagnostic::error(
                    CssCheckCategory::Parse,
                    "PARSE001",
                    "$HOME/.config/unixnotis/base.css".to_string(),
                    Some(12),
                    Some(4),
                    "expected a valid value",
                    Some("GTK CSS does not use var() custom properties here".to_string()),
                ),
                CssCheckDiagnostic::warning(
                    CssCheckCategory::Theme,
                    "THEME007",
                    "$HOME/.config/unixnotis/base.css".to_string(),
                    "css asset reference points outside $HOME/.config/unixnotis",
                ),
                CssCheckDiagnostic::warning(
                    CssCheckCategory::Lint,
                    "LINT002",
                    "$HOME/.config/unixnotis/widgets.css".to_string(),
                    "duplicate selector '.unixnotis-info-icon'",
                ),
                CssCheckDiagnostic::warning(
                    CssCheckCategory::Lint,
                    "LINT003",
                    "$HOME/.config/unixnotis/widgets.css".to_string(),
                    "duplicate property 'padding' in selector '.unixnotis-panel'",
                ),
            ],
        };

        let rendered = render_css_check_report_with_style(&report, ReportStyle { color: false });
        // These anchors keep the report layout easy to scan in the terminal
        let notes_idx = rendered.find("\nnotes\n").expect("notes section");
        let errors_idx = rendered.find("\nerrors\n").expect("errors section");
        let warnings_idx = rendered.find("\nwarnings\n").expect("warnings section");

        assert!(rendered.contains("top problem files"));
        assert!(rendered.contains("$HOME/.config/unixnotis/widgets.css: 2 issue(s)"));
        assert!(rendered.contains("[PARSE001][parse] line 12, col 4: expected a valid value"));
        assert!(rendered.contains("hint: GTK CSS does not use var() custom properties here"));
        assert!(rendered.contains("css-check result: failed"));
        assert!(notes_idx < errors_idx);
        assert!(errors_idx < warnings_idx);
    }

    #[test]
    fn render_report_sorts_diagnostics_by_severity_then_category() {
        // Errors should stay ahead of warnings, and warnings should keep the category order
        let report = CssCheckReport {
            display_root: "$HOME/.config/unixnotis".to_string(),
            checked_files: 5,
            diagnostics: vec![
                CssCheckDiagnostic::warning(
                    CssCheckCategory::Lint,
                    "LINT002",
                    "$HOME/.config/unixnotis/b.css".to_string(),
                    "duplicate selector '.b'",
                ),
                CssCheckDiagnostic::warning(
                    CssCheckCategory::Theme,
                    "THEME004",
                    "$HOME/.config/unixnotis/a.css".to_string(),
                    "1 configured theme file(s) point outside $HOME/.config/unixnotis",
                ),
                CssCheckDiagnostic::error(
                    CssCheckCategory::Parse,
                    "PARSE001",
                    "$HOME/.config/unixnotis/c.css".to_string(),
                    Some(2),
                    Some(1),
                    "expected a valid value",
                    None,
                ),
            ],
            ..CssCheckReport::default()
        };

        let rendered = render_css_check_report_with_style(&report, ReportStyle { color: false });
        let parse_idx = rendered
            .find("[PARSE001][parse]")
            .expect("parse diagnostic");
        let theme_idx = rendered
            .find("[THEME004][theme]")
            .expect("theme diagnostic");
        let lint_idx = rendered.find("[LINT002][lint]").expect("lint diagnostic");

        assert!(parse_idx < theme_idx);
        assert!(theme_idx < lint_idx);
    }

    #[test]
    fn render_report_can_add_terminal_color() {
        // Color should wrap the same plain text instead of changing the report content
        let report = CssCheckReport {
            display_root: "$HOME/.config/unixnotis".to_string(),
            checked_files: 5,
            diagnostics: vec![CssCheckDiagnostic::warning(
                CssCheckCategory::Lint,
                "LINT002",
                "$HOME/.config/unixnotis/widgets.css".to_string(),
                "duplicate selector '.unixnotis-info-icon'",
            )],
            ..CssCheckReport::default()
        };

        let rendered = render_css_check_report_with_style(&report, ReportStyle { color: true });
        assert!(rendered.contains("\u{1b}[1mcss-check summary\u{1b}[0m"));
        assert!(rendered.contains("\u{1b}[1;33mwarnings\u{1b}[0m"));
        assert!(rendered.contains("\u{1b}[2;35m[LINT002][lint]\u{1b}[0m"));
        assert!(rendered.contains("css-check result: \u{1b}[1;33mwarnings found\u{1b}[0m"));
    }
}
