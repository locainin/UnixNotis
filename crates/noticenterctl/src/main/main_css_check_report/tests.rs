use super::{
    model::{CssCheckActiveFile, CssCheckCategory, CssCheckDiagnostic, CssCheckReport},
    render::render_css_check_report_with_style,
    style::ReportStyle,
};

#[test]
fn render_report_groups_warnings_by_file_and_category() {
    // Grouping by file is the main readability win of the new report layout
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
    // Clean runs should not dump section headers that only matter during failures
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
