use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{List, ListItem};

use crate::model::{ActionStep, StepStatus};
use std::collections::VecDeque;

pub(super) fn render_steps(steps: &[ActionStep], width: u16) -> List<'static> {
    // Step list is compact and uses status tags for quick scanning.
    let inner_width = width.saturating_sub(2) as usize;
    let items = steps
        .iter()
        .map(|step| {
            let (symbol, style) = match step.status {
                StepStatus::Pending => ("[ ]", Style::default().fg(Color::Gray)),
                StepStatus::Running => ("[..]", Style::default().fg(Color::Yellow)),
                StepStatus::Done => ("[ok]", Style::default().fg(Color::Green)),
                StepStatus::Failed => ("[!!]", Style::default().fg(Color::Red)),
            };
            let available = inner_width.saturating_sub(symbol.len() + 1);
            let label = truncate_to_width(step.name, available);
            ListItem::new(Line::from(vec![
                Span::styled(symbol, style.add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::raw(label),
            ]))
        })
        .collect::<Vec<_>>();

    List::new(items)
}

pub(super) fn render_logs(logs: &VecDeque<String>, width: u16) -> Text<'static> {
    // Logs are wrapped to the available width to prevent horizontal scrolling.
    let inner_width = width.saturating_sub(2) as usize;
    let mut lines = Vec::new();
    for line in logs {
        for wrapped in wrap_line(line, inner_width) {
            let wrapped = truncate_to_width(&wrapped, inner_width);
            lines.push(Line::from(wrapped));
        }
    }
    Text::from(lines)
}

fn wrap_line(line: &str, width: usize) -> Vec<String> {
    // Zero-width means there is no space to render, return a single empty line.
    if width == 0 {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    // Track current width to avoid repeated O(n) scans as the line grows.
    let mut current_width = 0usize;
    // Tabs are normalized to spaces to keep alignment deterministic.
    let sanitized = line.replace('\t', " ");

    for word in sanitized.split_whitespace() {
        let word_width = word.chars().count();
        if word_width > width {
            if !current.is_empty() {
                lines.push(current);
                current = String::new();
                current_width = 0;
            }
            for chunk in break_long_word(word, width) {
                lines.push(chunk);
            }
            continue;
        }

        let next_len = if current.is_empty() {
            word_width
        } else {
            current_width + 1 + word_width
        };

        if next_len > width {
            lines.push(current);
            current = word.to_string();
            current_width = word_width;
        } else {
            if !current.is_empty() {
                current.push(' ');
                current_width += 1;
            }
            current.push_str(word);
            current_width += word_width;
        }
    }

    if current.is_empty() && lines.is_empty() {
        lines.push(String::new());
    } else if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn break_long_word(word: &str, width: usize) -> Vec<String> {
    // Splits a single word into multiple chunks so it can wrap in a fixed-width UI.
    if width == 0 {
        return vec![String::new()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;

    for ch in word.chars() {
        current.push(ch);
        current_width += 1;
        if current_width >= width {
            chunks.push(current);
            current = String::new();
            current_width = 0;
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

pub(super) fn truncate_to_width(text: &str, width: usize) -> String {
    // Truncate and append ellipsis so menus stay aligned.
    if width == 0 {
        return String::new();
    }
    let len = text.chars().count();
    if len <= width {
        return text.to_string();
    }
    if width <= 3 {
        return text.chars().take(width).collect();
    }
    let mut out = String::new();
    for ch in text.chars().take(width - 3) {
        out.push(ch);
    }
    out.push_str("...");
    out
}

pub(super) fn summarize_error(err: &str) -> String {
    // Provide a short, user-friendly error line for the UI while keeping full details in logs.
    if err.contains("failed to install") {
        return "failed to install binary (see logs)".to_string();
    }
    if err.contains("missing build artifact") {
        return "missing release binary (see logs)".to_string();
    }
    if err.contains("command failed: cargo") {
        return "cargo command failed (see logs)".to_string();
    }
    if err.contains("repository root not found") {
        return "repository root not found (see logs)".to_string();
    }

    const MAX_LEN: usize = 72;

    let mut out = String::new();
    for ch in err.chars().take(MAX_LEN) {
        out.push(ch);
    }
    if err.chars().count() > MAX_LEN {
        out.push_str("...");
    }

    out
}
