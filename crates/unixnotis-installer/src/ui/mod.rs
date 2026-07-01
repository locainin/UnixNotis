//! Ratatui drawing helpers for installer screens.

mod build_accel;
#[cfg(test)]
#[path = "tests/build_accel.rs"]
mod build_accel_tests;
mod confirm;
#[cfg(test)]
#[path = "tests/confirm.rs"]
mod confirm_tests;
mod header;
mod progress;
#[cfg(test)]
#[path = "tests/progress.rs"]
mod progress_tests;
mod reset;
#[cfg(test)]
#[path = "tests/reset.rs"]
mod reset_tests;
#[cfg(test)]
#[path = "tests/support.rs"]
mod test_support;
mod welcome;
#[cfg(test)]
#[path = "tests/welcome.rs"]
mod welcome_tests;
mod widgets;
#[cfg(test)]
#[path = "tests/widgets.rs"]
mod widgets_tests;

use ratatui::widgets::Clear;
use ratatui::Frame;

use crate::app::{App, Screen};

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    // Clear the frame each redraw to avoid artifacting when content shrinks.
    frame.render_widget(Clear, frame.area());
    match app.screen {
        Screen::Welcome => welcome::draw_welcome(frame, app),
        Screen::Confirm(mode) => confirm::draw_confirm(frame, app, mode),
        Screen::ResetMenu => reset::draw_reset_menu(frame, app),
        Screen::RestoreSelect => reset::draw_restore_select(frame, app),
        Screen::Progress(mode) => progress::draw_progress(frame, app, mode),
        Screen::BuildAccel => build_accel::draw_build_accel(frame, app),
    }
}
