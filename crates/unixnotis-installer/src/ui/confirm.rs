use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::model::ActionMode;

use super::header::draw_header;

pub(super) fn draw_confirm(frame: &mut Frame<'_>, app: &App, mode: ActionMode) {
    // Confirmation screen keeps the user on one page before running actions.
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_header(frame, layout[0]);

    // Content lines are built first, then rendered as a wrapped paragraph.
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
        Span::raw(crate::actions::summarize_owner(&app.detection.owner)),
    ]));

    lines.push(Line::from(vec![
        Span::styled(
            "Verification: ",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(if app.verify { "enabled" } else { "disabled" }),
    ]));

    // Blocked state is rendered inline so it is visible before execution.
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
    // Warn about destructive actions before proceeding.
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
    .alignment(ratatui::layout::Alignment::Center)
    .block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, layout[2]);
}
