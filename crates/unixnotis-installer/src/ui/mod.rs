//! Ratatui drawing helpers for installer screens.

mod build_accel;
mod confirm;
mod header;
mod progress;
mod welcome;
mod widgets;

use ratatui::widgets::Clear;
use ratatui::Frame;

use crate::app::{App, Screen};

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    // Clear the frame each redraw to avoid artifacting when content shrinks.
    frame.render_widget(Clear, frame.area());
    match app.screen {
        Screen::Welcome => welcome::draw_welcome(frame, app),
        Screen::Confirm(mode) => confirm::draw_confirm(frame, app, mode),
        Screen::Progress(mode) => progress::draw_progress(frame, app, mode),
        Screen::BuildAccel => build_accel::draw_build_accel(frame, app),
    }
}
