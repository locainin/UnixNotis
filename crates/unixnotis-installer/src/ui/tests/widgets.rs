use std::collections::VecDeque;

use crate::model::{ActionStep, StepStatus};

use super::test_support::{buffer_text, render_widget_buffer};

#[test]
fn truncate_to_width_handles_zero_small_and_ellipsis_widths() {
    // Width handling prevents dynamic content from resizing compact list cells
    assert_eq!(super::widgets::truncate_to_width("abcdef", 0), "");
    assert_eq!(super::widgets::truncate_to_width("abcdef", 2), "ab");
    assert_eq!(super::widgets::truncate_to_width("abcdef", 5), "ab...");
    assert_eq!(super::widgets::truncate_to_width("abc", 5), "abc");
}

#[test]
fn truncate_to_width_counts_unicode_chars_not_bytes() {
    let truncated = super::widgets::truncate_to_width("abéfg", 4);

    // Multibyte text must stay valid UTF-8 after truncation
    assert_eq!(truncated, "a...");
}

#[test]
fn summarize_error_prefers_known_short_messages_and_truncates_unknown_errors() {
    let known = super::widgets::summarize_error("command failed: cargo build");
    let unknown = super::widgets::summarize_error(&"x".repeat(80));
    let exact_limit = super::widgets::summarize_error(&"x".repeat(72));

    // Known failures stay readable; unknown failures stay bounded for the TUI
    assert_eq!(known, "cargo command failed (see logs)");
    assert_eq!(unknown, format!("{}...", "x".repeat(72)));
    assert_eq!(exact_limit, "x".repeat(72));
}

#[test]
fn summarize_error_covers_known_installer_failures() {
    let cases = [
        (
            "failed to install unixnotis-daemon",
            "failed to install binary (see logs)",
        ),
        (
            "missing build artifact unixnotis-center",
            "missing release binary (see logs)",
        ),
        (
            "repository root not found from cwd",
            "repository root not found (see logs)",
        ),
    ];

    for (input, expected) in cases {
        assert_eq!(super::widgets::summarize_error(input), expected);
    }
}

#[test]
fn render_logs_preserves_each_log_line() {
    let logs = VecDeque::from(["first".to_string(), "second".to_string()]);

    let rendered = format!("{:?}", super::widgets::render_logs(&logs));

    // Logs are rendered line-for-line so command diagnostics stay readable
    assert!(rendered.contains("first"));
    assert!(rendered.contains("second"));
}

#[test]
fn render_steps_includes_every_status_symbol() {
    let steps = [
        ActionStep {
            name: "pending",
            status: StepStatus::Pending,
        },
        ActionStep {
            name: "running",
            status: StepStatus::Running,
        },
        ActionStep {
            name: "done",
            status: StepStatus::Done,
        },
        ActionStep {
            name: "failed",
            status: StepStatus::Failed,
        },
    ];

    let rendered = format!("{:?}", super::widgets::render_steps(&steps, 40));

    // Status symbols are the fastest way to scan progress in a small terminal
    assert!(rendered.contains("[ ]"));
    assert!(rendered.contains("[..]"));
    assert!(rendered.contains("[ok]"));
    assert!(rendered.contains("[!!]"));
}

#[test]
fn render_steps_reserves_width_for_status_prefix() {
    let steps = [ActionStep {
        name: "abcdef",
        status: StepStatus::Done,
    }];

    let list = super::widgets::render_steps(&steps, 10);
    let rendered = buffer_text(&render_widget_buffer(list, 10, 1));

    // Width 10 leaves 3 cells after "[ok] "; arithmetic changes must not leak extra label text
    assert!(rendered.contains("[ok] abc"));
    assert!(!rendered.contains("[ok] a..."));
    assert!(!rendered.contains("[ok] ab..."));
}

#[test]
fn style_lookup_handles_multibyte_text_before_label() {
    use ratatui::style::{Color, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    let paragraph = Paragraph::new(Line::from(vec![
        Span::raw("é "),
        Span::styled("selected", Style::default().fg(Color::Cyan)),
    ]));
    let buffer = render_widget_buffer(paragraph, 20, 1);

    let style = super::test_support::style_for_text(&buffer, "selected");

    // Multibyte text before the label must not shift the style lookup by raw byte count
    assert_eq!(style.fg, Some(Color::Cyan));
}
