use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use super::header::draw_header;
use super::widgets::truncate_to_width;
use crate::app::App;
use crate::model::ResetAction;
use crate::paths::format_with_home;

pub(super) fn draw_reset_menu(frame: &mut Frame<'_>, app: &App) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_header(frame, layout[0]);

    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(6)])
        .split(layout[1]);

    // Summary text updates with the highlighted action so the user always sees
    // what will happen before confirming.
    let details = build_reset_summary(app);

    let info_block = Block::default().title("Summary").borders(Borders::ALL);
    frame.render_widget(
        Paragraph::new(Text::from(details))
            .block(info_block)
            .wrap(Wrap { trim: true }),
        body[0],
    );

    let menu = render_reset_menu(app, body[1].width);
    let menu_block = Block::default()
        .title("Reset actions")
        .borders(Borders::ALL);
    frame.render_widget(menu.block(menu_block), body[1]);

    let footer = Paragraph::new(Text::from(Line::from(vec![
        Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" = select  "),
        Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" = back"),
    ])))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, layout[2]);
}

fn render_reset_menu(app: &App, width: u16) -> List<'static> {
    let inner_width = width.saturating_sub(2) as usize;
    let items = ["Reset to defaults", "Restore backup", "Return to menu"];

    let list_items = items
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let label = truncate_to_width(label, inner_width);
            let style = if index == app.reset_menu_index {
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

pub(super) fn draw_restore_select(frame: &mut Frame<'_>, app: &App) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_header(frame, layout[0]);

    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(6)])
        .split(layout[1]);

    // The restore screen keeps instructions minimal so the backup list has space.
    let details = vec![
        Line::from(Span::styled(
            "Available backups",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::raw(
            "Select a backup directory to restore. Current files will be overwritten.",
        )),
        Line::from(Span::raw(
            "Backups are created by the Reset action and stored under $XDG_CONFIG_HOME/unixnotis.",
        )),
    ];

    let info_block = Block::default().title("Restore").borders(Borders::ALL);
    frame.render_widget(
        Paragraph::new(Text::from(details))
            .block(info_block)
            .wrap(Wrap { trim: true }),
        body[0],
    );

    let list = render_restore_menu(app, body[1].width);
    let list_block = Block::default().title("Backups").borders(Borders::ALL);
    frame.render_widget(list.block(list_block), body[1]);

    let footer = Paragraph::new(Text::from(Line::from(vec![
        Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" = restore  "),
        Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" = back"),
    ])))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, layout[2]);
}

fn render_restore_menu(app: &App, width: u16) -> List<'static> {
    let inner_width = width.saturating_sub(2) as usize;
    let mut items = Vec::new();

    if app.restore_backups.is_empty() {
        items.push("No backups found".to_string());
    } else {
        for path in &app.restore_backups {
            let rendered = format_with_home(path);
            items.push(rendered);
        }
    }

    let list_items = items
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let label = truncate_to_width(label, inner_width);
            let style = if index == app.restore_menu_index {
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

fn build_reset_summary(app: &App) -> Vec<Line<'static>> {
    // Match summary to the highlighted action for quick visual feedback.
    let (title, body, accent) = match app.reset_menu_index {
        0 => (
            "Reset to defaults",
            vec![
                "Overwrites config.toml and all theme CSS with bundled defaults.",
                "Creates a Backup-YYYY-MM-DD snapshot first.",
                "Backups are kept under $XDG_CONFIG_HOME/unixnotis (default: 3).",
            ],
            Color::Yellow,
        ),
        1 => (
            "Restore backup",
            vec![
                "Replaces current config.toml and theme CSS with a selected backup.",
                "Only backups created by the installer can be restored.",
                "Backups live under $XDG_CONFIG_HOME/unixnotis.",
            ],
            Color::Cyan,
        ),
        _ => (
            "Return to menu",
            vec!["Leave this screen without making changes."],
            Color::Gray,
        ),
    };

    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        "Summary",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        format!("-> {title}"),
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    for line in body {
        lines.push(Line::from(Span::raw(line)));
    }
    lines
}

pub(super) fn describe_reset_action(app: &App) -> String {
    match &app.reset_action {
        ResetAction::ResetDefaults => {
            "Resetting to defaults will overwrite config.toml and theme files.".to_string()
        }
        ResetAction::RestoreBackup { path } => {
            format!("Restoring from backup: {}", format_with_home(path))
        }
    }
}
