//! Panel auto-close wiring

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use gtk::prelude::*;

use crate::control::UiCommand;
use crate::ui::{hyprland, try_send_command, UiStateInit};

use super::super::widgets::PanelWidgets;

fn connect_blur_close(
    command_tx: tokio::sync::mpsc::Sender<UiCommand>,
    visible_flag: Arc<AtomicBool>,
    window: &gtk::ApplicationWindow,
) {
    window.connect_is_active_notify(move |window| {
        if visible_flag.load(Ordering::SeqCst) && !window.is_active() {
            // Window focus loss is only meaningful while the panel is visible
            try_send_command(&command_tx, UiCommand::ClosePanel);
        }
    });
}

pub(in crate::ui) fn connect_auto_close(
    panel: &PanelWidgets,
    init: &UiStateInit,
    visible_flag: Arc<AtomicBool>,
) {
    if init.config.panel.close_on_click_outside {
        let started =
            hyprland::start_active_window_watcher(init.event_tx.clone(), visible_flag.clone());
        if should_connect_blur_close(true, init.config.panel.close_on_blur, started) {
            // Hyprland watcher is preferred, but blur close is a safe fallback
            connect_blur_close(init.command_tx.clone(), visible_flag, &panel.window);
        }
    } else if should_connect_blur_close(false, init.config.panel.close_on_blur, false) {
        connect_blur_close(init.command_tx.clone(), visible_flag, &panel.window);
    }
}

#[cfg(test)]
#[path = "tests/autoclose.rs"]
mod tests;

pub(super) const fn should_connect_blur_close(
    close_on_click_outside: bool,
    close_on_blur: bool,
    watcher_started: bool,
) -> bool {
    // Blur is needed only when enabled and no active outside-click watcher owns closing
    close_on_blur && (!close_on_click_outside || !watcher_started)
}
