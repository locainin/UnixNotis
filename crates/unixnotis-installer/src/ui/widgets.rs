use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{List, ListItem};

use crate::model::{ActionStep, StepStatus};
use std::collections::VecDeque;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub(super) fn render_steps(steps: &[ActionStep], width: u16) -> List<'static> {
    // Step list is compact and uses status tags for quick scanning
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

pub(super) fn render_logs(
    logs: &VecDeque<String>,
    visible_rows: usize,
    width: usize,
) -> Text<'static> {
    if visible_rows == 0 || width == 0 {
        return Text::default();
    }

    // Select by rendered rows so one wrapped diagnostic cannot hide the actual final line
    let mut rows = VecDeque::with_capacity(visible_rows);
    for logical_line in logs.iter().rev() {
        let wrapped = wrap_log_line(logical_line, width);
        for row in wrapped.into_iter().rev() {
            if rows.len() == visible_rows {
                break;
            }
            rows.push_front(Line::from(row));
        }
        if rows.len() == visible_rows {
            break;
        }
    }
    Text::from(rows.into_iter().collect::<Vec<_>>())
}

pub(super) fn truncate_to_width(text: &str, width: usize) -> String {
    // Truncate and append ellipsis so menus stay aligned
    if width == 0 {
        return String::new();
    }
    let len = UnicodeWidthStr::width(text);
    if len <= width {
        return text.to_string();
    }
    if width <= 3 {
        return take_display_width(text, width);
    }
    let mut out = String::new();
    out.push_str(&take_display_width(text, width - 3));
    out.push_str("...");
    out
}

fn wrap_log_line(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }

    let mut rows = Vec::new();
    let mut row = String::new();
    let mut row_width = 0usize;
    for character in text.chars() {
        let character_width = character.width().unwrap_or(0);
        if row_width > 0 && row_width.saturating_add(character_width) > width {
            rows.push(std::mem::take(&mut row));
            row_width = 0;
        }
        // A glyph wider than the viewport is still kept so content is never silently dropped
        row.push(character);
        row_width = row_width.saturating_add(character_width);
    }
    if !row.is_empty() {
        rows.push(row);
    }
    rows
}

fn take_display_width(text: &str, width: usize) -> String {
    let mut used = 0usize;
    text.chars()
        .take_while(|character| {
            let character_width = character.width().unwrap_or(0);
            if used.saturating_add(character_width) > width {
                return false;
            }
            used = used.saturating_add(character_width);
            true
        })
        .collect()
}

pub(super) fn summarize_error(err: &str) -> String {
    const MAX_LEN: usize = 72;

    // Provide a short user-friendly error line while keeping full details in logs
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

    let mut out = String::new();
    for ch in err.chars().take(MAX_LEN) {
        out.push(ch);
    }
    if err.chars().count() > MAX_LEN {
        out.push_str("...");
    }

    out
}
