use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::actions::BuildAccelConfigStatus;
use crate::app::{App, BuildAccelMenuMode};

use super::header::draw_header;
use super::widgets::truncate_to_width;

pub(super) fn draw_build_accel(frame: &mut Frame<'_>, app: &App) {
    // Layout keeps the header fixed and splits body from choice list.
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(10)])
        .split(frame.area());

    draw_header(frame, layout[0]);

    let body_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(6)])
        .split(layout[1]);

    // Prompt is informational and opt-in; no automatic package installs are performed.
    let body = render_build_accel_body(app);
    let block = Block::default()
        .title("Build acceleration (optional)")
        .borders(Borders::ALL);
    frame.render_widget(
        Paragraph::new(body).block(block).wrap(Wrap { trim: true }),
        body_layout[0],
    );

    let choices = render_build_accel_menu(app, body_layout[1].width);
    let choices_block = Block::default().title("Choices").borders(Borders::ALL);
    frame.render_widget(choices.block(choices_block), body_layout[1]);
}

fn render_build_accel_body(app: &App) -> Text<'static> {
    // The body includes tool status, config status, and a compact install hint.
    let mut lines = vec![
        Line::from(vec![Span::styled(
            "Optional build acceleration",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("Use Up/Down and Enter to select a choice. Esc returns to the menu."),
        Line::from(""),
    ];

    let Some(state) = app.build_accel.as_ref() else {
        lines.push(Line::from("Detection unavailable."));
        return Text::from(lines);
    };

    lines.push(package_status_line(
        "sccache",
        state.detection.sccache_installed,
        "compiler output cache (speeds rebuilds)",
    ));
    lines.push(package_status_line(
        "mold",
        state.detection.mold_installed,
        "fast linker (speeds link steps)",
    ));
    lines.push(build_config_status_line(&state.detection));

    // Keep configuration guidance short so the status block stays readable.
    lines.push(Line::from(""));
    lines.push(Line::from(
        "Configuration stays local and falls back if tools are removed.",
    ));

    if !state.detection.sccache_installed || !state.detection.mold_installed {
        // Use a single pacman invocation to reduce copy/paste friction.
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Install on Arch (manual):",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from("sudo pacman -S sccache mold"));
        lines.push(Line::from("Root permissions required by pacman."));
    }

    if let Some(outcome) = &state.outcome {
        // Show the most recent action result to make success/failure explicit.
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Result:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(outcome_summary(outcome)));
    }

    Text::from(lines)
}

fn package_status_line(name: &str, installed: bool, purpose: &str) -> Line<'static> {
    // Format: "pkg: status - purpose" to keep the list scannable.
    let status = if installed { "installed" } else { "missing" };
    let status_style = if installed {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Red)
    };

    // Status is bolded for quick scanning in noisy terminal themes.
    Line::from(vec![
        Span::styled(
            format!("{name}: "),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(status, status_style.add_modifier(Modifier::BOLD)),
        Span::raw(" - "),
        Span::raw(purpose.to_string()),
    ])
}

fn build_config_status_line(detection: &crate::actions::BuildAccelDetection) -> Line<'static> {
    // Config status is derived from installer-managed markers and tool availability.
    let (status_text, status_style) = match &detection.config_status {
        BuildAccelConfigStatus::Missing => {
            ("not installed".to_string(), Style::default().fg(Color::Red))
        }
        BuildAccelConfigStatus::Unmanaged => (
            "present (not managed by installer)".to_string(),
            Style::default().fg(Color::Yellow),
        ),
        BuildAccelConfigStatus::ReadFailed(err) => (
            format!("read failed ({err})"),
            Style::default().fg(Color::Red),
        ),
        BuildAccelConfigStatus::Managed { wrapper_present } => {
            if !*wrapper_present {
                (
                    "installed (wrapper missing)".to_string(),
                    Style::default().fg(Color::Yellow),
                )
            } else {
                // Tool availability determines the "installed for X" message.
                let tool_status = match (detection.sccache_installed, detection.mold_installed) {
                    (true, true) => "installed - sccache + mold",
                    (true, false) => "installed - sccache",
                    (false, true) => "installed - mold",
                    (false, false) => "installed - tools missing",
                };
                (tool_status.to_string(), Style::default().fg(Color::Green))
            }
        }
    };

    Line::from(vec![
        Span::styled("config: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(status_text, status_style.add_modifier(Modifier::BOLD)),
    ])
}

fn render_build_accel_menu(app: &App, width: u16) -> List<'static> {
    let inner_width = width.saturating_sub(2) as usize;
    // Menu choices depend on config state and tool availability to avoid invalid actions.
    let entries: Vec<&str> = match app.build_accel_menu_mode() {
        BuildAccelMenuMode::ReturnOnly => vec!["Return to menu"],
        BuildAccelMenuMode::EnableOrSkip => vec![
            "Enable build acceleration (.cargo/config.toml)",
            "Skip and return to menu",
        ],
        BuildAccelMenuMode::Reinstall => {
            vec!["Return to menu", "Reinstall build acceleration config"]
        }
    };
    // The first entry is always the default selection.
    let list_items = entries
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let label = truncate_to_width(label, inner_width);
            let style = if index == app.build_accel_menu_index {
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

fn outcome_summary(outcome: &crate::actions::BuildAccelOutcome) -> String {
    // Outcome text is short so it fits within the body block without wrapping.
    match outcome {
        crate::actions::BuildAccelOutcome::SkippedMissingTools => {
            "No build accelerators detected; enable unavailable.".to_string()
        }
        crate::actions::BuildAccelOutcome::SkippedExistingConfig => {
            "Existing config not managed by installer; no changes applied.".to_string()
        }
        crate::actions::BuildAccelOutcome::Written {
            relative_path,
            used_sccache,
            used_mold,
        } => format!(
            "Wrote {} (sccache={}, mold={}).",
            relative_path, used_sccache, used_mold
        ),
        crate::actions::BuildAccelOutcome::UpdatedExisting {
            relative_path,
            used_sccache,
            used_mold,
        } => format!(
            "Updated {} (sccache={}, mold={}).",
            relative_path, used_sccache, used_mold
        ),
        crate::actions::BuildAccelOutcome::Failed(err) => {
            format!("Setup failed: {err}")
        }
    }
}
