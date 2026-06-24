use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::model::ActionMode;
use crate::paths::format_with_home;

use super::header::draw_header;
use super::reset::describe_reset_action;

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
        let service_artifact = crate::paths::InstallPaths::discover()
            .map(|paths| paths.service.artifact_label())
            .unwrap_or("service artifact");
        lines.push(Line::from(Span::styled(
            format!("Reinstall will overwrite binaries and the {service_artifact}."),
            Style::default().fg(Color::Yellow),
        )));
    }
    if matches!(mode, ActionMode::Install | ActionMode::Uninstall) {
        lines.push(Line::from(""));
        // Fall back to the default user bin path when discovery cannot run here
        let bin_dir = crate::paths::InstallPaths::discover()
            .map(|paths| format_with_home(&paths.bin_dir))
            .unwrap_or_else(|_| "$HOME/.local/bin".to_string());
        if matches!(mode, ActionMode::Install) {
            // Install builds release binaries and copies them into the user bin dir
            lines.push(Line::from(Span::styled(
                format!(
                    "Install builds UnixNotis and copies unixnotis-daemon, unixnotis-popups, unixnotis-center, and noticenterctl into {}; startup files are updated so new terminals include this path",
                    bin_dir
                ),
                Style::default().fg(Color::Yellow),
            )));
        } else {
            // Uninstall removes the managed binaries from the same user bin dir
            lines.push(Line::from(Span::styled(
                format!(
                    "Uninstall removes unixnotis-daemon, unixnotis-popups, unixnotis-center, and noticenterctl from {}",
                    bin_dir
                ),
                Style::default().fg(Color::Yellow),
            )));
        }
    }
    if matches!(mode, ActionMode::Reset) {
        lines.push(Line::from(""));
        // Reset warning includes backup retention to avoid surprise data loss.
        lines.push(Line::from(Span::styled(
            "Reset overwrites config.toml and theme files with defaults.",
            Style::default().fg(Color::Yellow),
        )));
        lines.push(Line::from(Span::styled(
            "Existing files are backed up to $XDG_CONFIG_HOME/unixnotis/Backup-YYYY-MM-DD (keeps last 3; installer.toml).",
            Style::default().fg(Color::Yellow),
        )));
        lines.push(Line::from(Span::styled(
            describe_reset_action(app),
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
