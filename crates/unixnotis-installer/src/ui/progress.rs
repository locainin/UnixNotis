use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, ProgressState};
use crate::model::{ActionMode, ResetAction};

use super::header::draw_header;
use super::widgets::{render_logs, render_steps, summarize_error};

pub(super) fn draw_progress(frame: &mut Frame<'_>, app: &App, mode: ActionMode) {
    // Progress screen emphasizes current state and separates steps/logs.
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

    // Build the status block first so error summaries stay close to the header.
    let action_label = match mode {
        ActionMode::Reset => match &app.reset_action {
            ResetAction::ResetDefaults => "Reset config",
            ResetAction::RestoreBackup { .. } => "Restore backup",
        },
        _ => app.action_label(mode),
    };
    let mut status_lines = vec![Line::from(Span::styled(
        format!("{action_label} - {status_label}"),
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

    // Status block remains centered to keep user focus on action state.
    let status = Paragraph::new(Text::from(status_lines))
        .alignment(Alignment::Center)
        .block(Block::default().title("Progress").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(status, layout[1]);

    // Body splits step list and logs to keep both visible during execution.
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

    // Footer text varies by state to make next action explicit.
    let footer_text = match app.progress_state {
        ProgressState::Running => "Running...",
        ProgressState::Completed | ProgressState::Failed => {
            if matches!(mode, ActionMode::Install)
                && matches!(app.progress_state, ProgressState::Completed)
            {
                "Enter = build acceleration options  Q = quit"
            } else {
                "Enter = back to menu  Q = quit"
            }
        }
        ProgressState::Idle => "",
    };
    let footer = Paragraph::new(footer_text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, layout[3]);
}
