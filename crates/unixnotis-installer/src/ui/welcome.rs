use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use super::header::draw_header;
use super::widgets::truncate_to_width;
use crate::actions::{
    daemon_has_displayable_status, daemon_status_is_warning, format_daemon_status, summarize_owner,
    InstallationDisposition,
};
use crate::app::{App, MenuItem};
use crate::checks::{CheckItem, CheckState};

pub(super) fn draw_welcome(frame: &mut Frame<'_>, app: &App) {
    // Welcome layout: header, main split (status/menu), and a short footer.
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

    // Status is rendered as a text block to keep it stable across refreshes.
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

    // Menu width is derived from the layout to avoid overflow in narrow terminals.
    let menu = render_menu(app, body[1].width);
    let menu_block = Block::default().title("Actions").borders(Borders::ALL);
    frame.render_widget(menu.block(menu_block), body[1]);

    // Footer is dense to avoid wasting vertical space on hints.
    let footer = Paragraph::new(Text::from(Line::from(vec![
        Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" = select  "),
        Span::styled("Up/Down", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" = move  "),
        Span::styled("R", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" = refresh  "),
        Span::styled("Q", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" = quit"),
    ])))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, layout[2]);
}

fn render_status(app: &App) -> Text<'static> {
    // Build a list of Lines that ratatui will render as a single Text block.
    // This is kept pure so rendering remains deterministic for any given App state.
    let disposition = app.installation_disposition();
    let (version, version_role) = app
        .install_state
        .as_ref()
        .and_then(crate::actions::InstallState::installed_version)
        .map_or_else(
            || (app.release_status.current.clone(), "installer"),
            |installed_version| {
                (
                    format!("v{}", installed_version.trim_start_matches('v')),
                    installed_version_role(disposition),
                )
            },
        );
    let mut lines = vec![
        // Section heading: release version and update status.
        Line::from(Span::styled(
            "Release",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("Version: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                app.release_status.display_line_for(&version, version_role),
                release_status_style(app.release_status.update_state_for(&version)),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "Install state: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(disposition.label()),
        ]),
        Line::from(""),
        // Section heading: core environment checks.
        Line::from(Span::styled(
            "Compatibility",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];
    lines.extend(render_check(&app.checks.wayland));
    lines.extend(render_check(&app.checks.hyprland));
    lines.extend(render_check(&app.checks.service_manager));
    lines.extend(render_check(&app.checks.cargo));
    lines.extend(render_check(&app.checks.pkg_config));
    lines.extend(render_check(&app.checks.gtk4_css_features));
    lines.extend(render_check(&app.checks.gtk4_layer_shell));
    lines.extend(render_check(&app.checks.busctl));
    lines.extend(render_check(&app.checks.dbus_update_env));
    lines.extend(render_check(&app.checks.install_paths));
    lines.extend(render_check(&app.checks.path_contains_bin));

    render_daemon_section(app, &mut lines);

    Text::from(lines)
}

pub(super) const fn installed_version_role(disposition: InstallationDisposition) -> &'static str {
    // Only a fully verified generation may use the unqualified installed wording
    if matches!(disposition, InstallationDisposition::InstalledHealthy) {
        "installed"
    } else {
        "binaries present"
    }
}

fn render_daemon_section(app: &App, lines: &mut Vec<Line<'static>>) {
    let visible_daemons = app
        .detection
        .daemons
        .iter()
        .filter(|daemon| daemon_has_displayable_status(daemon))
        .collect::<Vec<_>>();

    if app.detection.owner.is_none() && visible_daemons.is_empty() {
        // Empty owner plus inactive daemon probes should not burn the first screen
        return;
    }

    // Visual separator appears only when there is a real second section to show
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Notification daemons",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled("Owner: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(
            summarize_owner(app.detection.owner.as_ref()),
            daemon_owner_style(app.detection.owner.is_some()),
        ),
    ]));

    for daemon in visible_daemons {
        // Rows that survive filtering are useful signals, so color the result like checks
        lines.push(Line::from(vec![
            Span::styled(
                format!("{}: ", daemon.name),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(format_daemon_status(daemon), daemon_status_style(daemon)),
        ]));
    }
}

fn daemon_owner_style(has_owner: bool) -> Style {
    if has_owner {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Yellow)
    }
}

fn release_status_style(state: crate::release::ReleaseUpdateState) -> Style {
    match state {
        // Green means the currently installed version matches the latest stable release
        crate::release::ReleaseUpdateState::UpToDate => Style::default().fg(Color::Green),
        // Yellow keeps updates visible without making the installer look blocked
        crate::release::ReleaseUpdateState::UpdateAvailable => Style::default().fg(Color::Yellow),
        // Unknown is expected offline, in CI, or when GitHub rate-limits the unauthenticated check
        crate::release::ReleaseUpdateState::Unknown => Style::default().fg(Color::Gray),
    }
}

fn daemon_status_style(daemon: &crate::detect::DetectedDaemon) -> Style {
    if daemon_status_is_warning(daemon) {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Green)
    }
}

fn render_check(item: &CheckItem) -> Vec<Line<'static>> {
    // Map check state -> short label plus color for rapid scanning.
    let (symbol, style) = match item.state {
        CheckState::Ok => ("[ok]", Style::default().fg(Color::Green)),
        CheckState::Warn => ("[warn]", Style::default().fg(Color::Yellow)),
        CheckState::Fail => ("[fail]", Style::default().fg(Color::Red)),
    };

    // Format: [ok] Label - detail
    vec![Line::from(vec![
        Span::styled(symbol, style.add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(item.label, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" - "),
        Span::raw(item.detail.clone()),
    ])]
}

fn render_menu(app: &App, width: u16) -> List<'static> {
    // Truncate labels to keep the list aligned in small terminals.
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
