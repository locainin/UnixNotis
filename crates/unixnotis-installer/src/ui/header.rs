use ratatui::layout::Alignment;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{layout::Rect, Frame};

pub(super) fn draw_header(frame: &mut Frame<'_>, area: Rect) {
    // The header is shared across screens to keep the installer identity consistent.
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
