//! UI loop and event dispatch for the installer TUI.

use anyhow::Result;
use crossterm::event::{self, Event};
use std::sync::mpsc;
use std::thread;

use crate::action_workflow::apply_worker_event;
use crate::app::{App, Screen};
use crate::events::UiMessage;
use crate::main_handlers::{
    handle_build_accel_key, handle_confirm_key, handle_progress_key, handle_reset_menu_key,
    handle_restore_select_key, handle_welcome_key,
};
use crate::terminal::TerminalGuard;
use crate::ui;
use crate::ExitAction;

pub(crate) fn run_app(terminal_guard: &mut TerminalGuard, app: &mut App) -> Result<ExitAction> {
    // Bound UI event channel to avoid unbounded memory growth if worker output
    // (especially verbose logs) outpaces the render loop.
    const UI_QUEUE_CAPACITY: usize = 512;
    let (ui_tx, ui_rx) = mpsc::sync_channel::<UiMessage>(UI_QUEUE_CAPACITY);
    spawn_input_thread(ui_tx.clone());

    terminal_guard
        .terminal_mut()
        .draw(|frame| ui::draw(frame, app))?;

    loop {
        match ui_rx.recv() {
            Ok(UiMessage::Input(input)) => {
                if let Some(exit) = handle_event(app, terminal_guard, &ui_tx, input)? {
                    return Ok(exit);
                }
            }
            Ok(UiMessage::Worker(event)) => {
                apply_worker_event(app, event);
            }
            Err(_) => return Ok(ExitAction::None),
        }

        terminal_guard
            .terminal_mut()
            .draw(|frame| ui::draw(frame, app))?;
    }
}

fn handle_event(
    app: &mut App,
    terminal_guard: &mut TerminalGuard,
    ui_tx: &mpsc::SyncSender<UiMessage>,
    event: Event,
) -> Result<Option<ExitAction>> {
    match event {
        Event::Key(key) => match app.screen {
            Screen::Welcome => handle_welcome_key(app, key),
            Screen::Confirm(mode) => handle_confirm_key(app, terminal_guard, ui_tx, key, mode),
            Screen::ResetMenu => handle_reset_menu_key(app, key),
            Screen::RestoreSelect => handle_restore_select_key(app, key),
            Screen::Progress(_) => handle_progress_key(app, key),
            Screen::BuildAccel => handle_build_accel_key(app, key),
        },
        Event::Resize(_, _) => Ok(None),
        _ => Ok(None),
    }
}

fn spawn_input_thread(ui_tx: mpsc::SyncSender<UiMessage>) {
    // Forward blocking terminal events to the UI thread; exit on channel close.
    thread::spawn(move || {
        while let Ok(event) = event::read() {
            if ui_tx.send(UiMessage::Input(event)).is_err() {
                break;
            }
        }
    });
}
