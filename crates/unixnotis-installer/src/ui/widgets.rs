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

pub(super) fn render_logs(logs: &VecDeque<String>) -> Text<'static> {
    // Use Ratatui's native wrapping to avoid per-frame manual wrapping allocations.
    let lines: Vec<Line<'static>> = logs.iter().map(|line| Line::from(line.clone())).collect();
    Text::from(lines)
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
