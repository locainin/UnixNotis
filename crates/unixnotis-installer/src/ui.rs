//! Ratatui drawing helpers for installer screens.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::actions::{format_daemon_status, summarize_owner};
use crate::app::{App, MenuItem, ProgressState, Screen};
use crate::checks::{CheckItem, CheckState};
use crate::model::{ActionMode, ActionStep, StepStatus};

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    frame.render_widget(Clear, frame.area());
    match app.screen {
        Screen::Welcome => draw_welcome(frame, app),
        Screen::Confirm(mode) => draw_confirm(frame, app, mode),
        Screen::Progress(mode) => draw_progress(frame, app, mode),
    }
}

fn draw_welcome(frame: &mut Frame<'_>, app: &App) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_header(frame, layout[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(layout[1]);

    let status = render_status(app);
    let status_block = Block::default()
        .title("System status")
        .borders(Borders::ALL);
    frame.render_widget(
        Paragraph::new(status)
            .block(status_block)
            .wrap(Wrap { trim: true }),
        body[0],
    );

    let menu = render_menu(app, body[1].width);
    let menu_block = Block::default().title("Actions").borders(Borders::ALL);
    frame.render_widget(menu.block(menu_block), body[1]);

    let footer = Paragraph::new(Text::from(Line::from(vec![
        Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" = select  "),
        Span::styled("Up/Down", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" = move  "),
        Span::styled("R", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" = refresh  "),
        Span::styled("V", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" = toggle verify  "),
        Span::styled("Q", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" = quit"),
    ])))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, layout[2]);
}

fn draw_confirm(frame: &mut Frame<'_>, app: &App, mode: ActionMode) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_header(frame, layout[0]);

    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        format!("Confirm {}", app.action_label(mode)),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(""));

    lines.push(Line::from(vec![
        Span::styled(
            "Current owner: ",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(summarize_owner(&app.detection.owner)),
    ]));

    lines.push(Line::from(vec![
        Span::styled(
            "Verification: ",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(if app.verify { "enabled" } else { "disabled" }),
    ]));

    if let Err(reason) = app.checks.ready_for(mode) {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                "Blocked: ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(reason),
        ]));
    }
    if matches!(mode, ActionMode::Install)
        && app
            .install_state
            .as_ref()
            .map(|state| state.is_fully_installed())
            .unwrap_or(false)
    {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Reinstall will overwrite binaries and the systemd unit.",
            Style::default().fg(Color::Yellow),
        )));
    }
    if matches!(mode, ActionMode::Reset) {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Reset will overwrite config.toml and theme files with defaults.",
            Style::default().fg(Color::Yellow),
        )));
    }

    let block = Block::default().title("Confirmation").borders(Borders::ALL);
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(block)
            .wrap(Wrap { trim: true }),
        layout[1],
    );

    let footer = Paragraph::new(Text::from(Line::from(vec![
        Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" = proceed  "),
        Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" = cancel"),
    ])))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, layout[2]);
}

fn draw_progress(frame: &mut Frame<'_>, app: &App, mode: ActionMode) {
    let (status_label, status_color) = match app.progress_state {
        ProgressState::Running => ("In progress", Color::Yellow),
        ProgressState::Completed => ("Completed", Color::Green),
        ProgressState::Failed => ("Failed", Color::Red),
        ProgressState::Idle => ("Pending", Color::Gray),
    };

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(6),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_header(frame, layout[0]);

    let mut status_lines = vec![Line::from(Span::styled(
        format!("{} - {}", app.action_label(mode), status_label),
        Style::default()
            .fg(status_color)
            .add_modifier(Modifier::BOLD),
    ))];
    if let Some(err) = &app.last_error {
        if matches!(app.progress_state, ProgressState::Failed) {
            let summary = summarize_error(err);
            status_lines.push(Line::from(vec![
                Span::styled("Error: ", Style::default().fg(Color::Red)),
                Span::raw(summary),
            ]));
            status_lines.push(Line::from("See logs for full output."));
        }
    }

    let status = Paragraph::new(Text::from(status_lines))
        .alignment(Alignment::Center)
        .block(Block::default().title("Progress").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(status, layout[1]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(layout[2]);

    let steps = render_steps(&app.steps, body[0].width);
    let steps_block = Block::default().title("Steps").borders(Borders::ALL);
    frame.render_widget(steps.block(steps_block), body[0]);

    let logs = render_logs(&app.logs, body[1].width);
    let logs_block = Block::default().title("Logs").borders(Borders::ALL);
    frame.render_widget(Paragraph::new(logs).block(logs_block), body[1]);

    let footer_text = match app.progress_state {
        ProgressState::Running => "Running...",
        ProgressState::Completed | ProgressState::Failed => "Enter = back to menu  Q = quit",
        ProgressState::Idle => "",
    };
    let footer = Paragraph::new(footer_text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, layout[3]);
}

fn draw_header(frame: &mut Frame<'_>, area: Rect) {
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            "UnixNotis Installer",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  —  Arch Wayland Notification Center"),
    ]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, area);
}

fn render_status(app: &App) -> Text<'static> {
    // Build a list of Lines that ratatui will render as a single Text block.
    // We keep this as "pure rendering": it only reads state from `app` and formats it.
    let mut lines = Vec::new();

    // Section header: Compatibility / environment checks.
    lines.push(Line::from(Span::styled(
        "Compatibility",
        Style::default().add_modifier(Modifier::BOLD),
    )));

    // Each check is rendered as a single line with a colored status tag and a detail message.
    lines.extend(render_check(&app.checks.wayland));
    lines.extend(render_check(&app.checks.hyprland));
    lines.extend(render_check(&app.checks.systemd_user));
    lines.extend(render_check(&app.checks.cargo));
    lines.extend(render_check(&app.checks.busctl));

    // Blank line = visual separation between sections.
    lines.push(Line::from(""));

    // Section header: notification daemon detection + current bus owner.
    lines.push(Line::from(Span::styled(
        "Notification daemons",
        Style::default().add_modifier(Modifier::BOLD),
    )));

    // Show who currently owns the org.freedesktop.Notifications bus name.
    lines.push(Line::from(vec![
        Span::styled("Owner: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(summarize_owner(&app.detection.owner)),
    ]));

    // List detected daemons and their status.
    // The name is highlighted; the status text explains running/stopped/managed/etc.
    for daemon in &app.detection.daemons {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{}: ", daemon.name),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw(format_daemon_status(daemon)),
        ]));
    }

    // Another section break.
    lines.push(Line::from(""));

    // Whether extra verification is enabled (affects action plan and/or checks).
    lines.push(Line::from(vec![
        Span::styled(
            "Verification: ",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(if app.verify { "enabled" } else { "disabled" }),
    ]));

    // Convert the collected lines into a ratatui Text.
    Text::from(lines)
}

fn render_check(item: &CheckItem) -> Vec<Line<'static>> {
    // Map check state -> a short label plus color.
    // Bolding the tag makes it stand out even in crowded terminal themes.
    let (symbol, style) = match item.state {
        CheckState::Ok => ("[ok]", Style::default().fg(Color::Green)),
        CheckState::Warn => ("[warn]", Style::default().fg(Color::Yellow)),
        CheckState::Fail => ("[fail]", Style::default().fg(Color::Red)),
    };

    // Render format:
    // [ok] <Label> - <detail>
    // detail is cloned because Span::raw needs an owned String for 'static lines here.
    vec![Line::from(vec![
        Span::styled(symbol, style.add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(item.label, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" - "),
        Span::raw(item.detail.clone()),
    ])]
}

fn render_menu(app: &App, width: u16) -> List<'static> {
    let inner_width = width.saturating_sub(2) as usize;
    let items = App::menu_items();
    let list_items = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let label = match item {
                MenuItem::Action(mode) => app.action_label(*mode),
                MenuItem::Quit => "Quit",
            };
            let label = truncate_to_width(label, inner_width);
            let style = if index == app.menu_index {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(label, style)))
        })
        .collect::<Vec<_>>();

    List::new(list_items)
}

fn render_steps(steps: &[ActionStep], width: u16) -> List<'static> {
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

fn render_logs(logs: &[String], width: u16) -> Text<'static> {
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
    if width == 0 {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    let sanitized = line.replace('\t', " ");

    for word in sanitized.split_whitespace() {
        let word_width = word.chars().count();
        if word_width > width {
            if !current.is_empty() {
                lines.push(current);
                current = String::new();
            }
            for chunk in break_long_word(word, width) {
                lines.push(chunk);
            }
            continue;
        }

        let next_len = if current.is_empty() {
            word_width
        } else {
            current.chars().count() + 1 + word_width
        };

        if next_len > width {
            lines.push(current);
            current = word.to_string();
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
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
    // Splits a single "word" into multiple chunks so it can wrap in a fixed-width UI.
    // This is used when the normal word-wrapping logic can't break on whitespace.
    if width == 0 {
        // Avoid division-by-zero style behavior; nothing fits, so return a single empty chunk.
        return vec![String::new()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    // Iterate by Unicode scalar values (chars), so we never cut UTF-8 in the middle of a codepoint.
    // Note: this counts "chars", not terminal column width (wide glyphs/emojis may still misalign).
    for ch in word.chars() {
        current.push(ch);

        // When the current chunk reaches the target width, finalize it and start a new one.
        // This uses a char count so it matches the way we built the string above.
        if current.chars().count() >= width {
            chunks.push(current);
            current = String::new();
        }
    }

    // Push any remaining tail chunk.
    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

fn truncate_to_width(text: &str, width: usize) -> String {
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

fn summarize_error(err: &str) -> String {
    // Provide a short, user-friendly error line for the UI while keeping full details in logs.
    // This prevents the status panel from being dominated by long multi-line errors.

    // Special-case: errors that match a known failure mode get a stable short summary.
    // (This makes the UI consistent even if the underlying error text changes slightly.)
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

    // Default: truncate to a fixed number of characters and add an ellipsis if needed.
    const MAX_LEN: usize = 72;

    // Build output by chars so we don't split UTF-8 sequences.
    let mut out = String::new();
    for ch in err.chars().take(MAX_LEN) {
        out.push(ch);
    }

    // If the original string is longer, append "...".
    // Note: err.chars().count() is O(n), but MAX_LEN is small and this runs rarely.
    if err.chars().count() > MAX_LEN {
        out.push_str("...");
    }

    out
}
