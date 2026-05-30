//! Key handling for each installer screen.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::mpsc;
use std::time::Instant;

use crate::action_workflow::{
    handle_build_accel_enter, prepare_build_accel_prompt, reset_to_menu, start_action,
};
use crate::app::{App, MenuItem, ProgressState, Screen};
use crate::events::UiMessage;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::terminal::TerminalGuard;
use crate::ExitAction;

pub(crate) fn handle_welcome_key(app: &mut App, key: KeyEvent) -> Result<Option<ExitAction>> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => Ok(Some(ExitAction::None)),
        KeyCode::Up | KeyCode::Char('k') => {
            if app.menu_index > 0 {
                app.menu_index -= 1;
            }
            Ok(None)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.menu_index + 1 < App::menu_items().len() {
                app.menu_index += 1;
            }
            Ok(None)
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            app.refresh();
            Ok(None)
        }
        KeyCode::Enter => match app.selected_menu() {
            MenuItem::Quit => Ok(Some(ExitAction::None)),
            MenuItem::Action(mode) => {
                if mode == ActionMode::Reset {
                    // Reset uses a submenu to avoid accidental destructive actions.
                    app.reset_menu_index = 0;
                    app.screen = Screen::ResetMenu;
                } else {
                    app.screen = Screen::Confirm(mode);
                }
                Ok(None)
            }
        },
        _ => Ok(None),
    }
}

pub(crate) fn handle_reset_menu_key(app: &mut App, key: KeyEvent) -> Result<Option<ExitAction>> {
    match key.code {
        KeyCode::Esc => {
            app.screen = Screen::Welcome;
            Ok(None)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            // Clamp selection to keep navigation predictable in small terminals.
            if app.reset_menu_index > 0 {
                app.reset_menu_index -= 1;
            }
            Ok(None)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            // Reset menu has three entries; enforce bounds.
            if app.reset_menu_index < 2 {
                app.reset_menu_index += 1;
            }
            Ok(None)
        }
        KeyCode::Enter => {
            match app.reset_menu_index {
                0 => {
                    // Default reset overwrites config and theme files.
                    app.reset_action = crate::model::ResetAction::ResetDefaults;
                    app.screen = Screen::Confirm(ActionMode::Reset);
                }
                1 => {
                    // Restore flow needs the latest list of backup directories.
                    app.refresh_backups();
                    app.screen = Screen::RestoreSelect;
                }
                _ => {
                    app.screen = Screen::Welcome;
                }
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

pub(crate) fn handle_restore_select_key(
    app: &mut App,
    key: KeyEvent,
) -> Result<Option<ExitAction>> {
    match key.code {
        KeyCode::Esc => {
            app.screen = Screen::ResetMenu;
            Ok(None)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            // Backup selection should never underflow.
            if app.restore_menu_index > 0 {
                app.restore_menu_index -= 1;
            }
            Ok(None)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            // Only advance selection when there are backup entries.
            if app.restore_menu_index + 1 < app.restore_backups.len() {
                app.restore_menu_index += 1;
            }
            Ok(None)
        }
        KeyCode::Enter => {
            // Restore proceeds only when a backup is selected.
            if let Some(path) = app.restore_backups.get(app.restore_menu_index).cloned() {
                app.reset_action = crate::model::ResetAction::RestoreBackup { path };
                app.screen = Screen::Confirm(ActionMode::Reset);
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

pub(crate) fn handle_confirm_key(
    app: &mut App,
    terminal_guard: &mut TerminalGuard,
    ui_tx: &mpsc::SyncSender<UiMessage>,
    key: KeyEvent,
    mode: ActionMode,
) -> Result<Option<ExitAction>> {
    match key.code {
        KeyCode::Esc => {
            app.screen = Screen::Welcome;
            Ok(None)
        }
        KeyCode::Enter => {
            if let Err(reason) = app.checks.ready_for(mode) {
                app.last_error = Some(reason);
                app.progress_state = ProgressState::Failed;
                app.logs.clear();
                app.steps.clear();
                app.screen = Screen::Progress(mode);
                return Ok(None);
            }

            match mode {
                ActionMode::Test => {
                    let paths = InstallPaths::discover()?;
                    return Ok(Some(ExitAction::RunTrial {
                        repo_root: paths.repo_root.clone(),
                    }));
                }
                ActionMode::Install | ActionMode::Uninstall | ActionMode::Reset => {
                    start_action(app, terminal_guard, ui_tx, mode)?;
                }
            }

            Ok(None)
        }
        _ => Ok(None),
    }
}

pub(crate) fn handle_progress_key(app: &mut App, key: KeyEvent) -> Result<Option<ExitAction>> {
    if matches!(app.progress_state, ProgressState::Running) {
        return Ok(None);
    }
    if let Some(ready_at) = app.progress_ready_at {
        if Instant::now() < ready_at {
            return Ok(None);
        }
    }
    match key.code {
        KeyCode::Enter => {
            if matches!(app.screen, Screen::Progress(ActionMode::Install))
                && matches!(app.progress_state, ProgressState::Completed)
            {
                // Present the optional build-acceleration prompt after a successful install.
                prepare_build_accel_prompt(app);
                app.screen = Screen::BuildAccel;
            } else {
                reset_to_menu(app);
            }
            Ok(None)
        }
        KeyCode::Char('q') | KeyCode::Char('Q') => Ok(Some(ExitAction::None)),
        KeyCode::Esc => {
            app.screen = Screen::Welcome;
            Ok(None)
        }
        _ => Ok(None),
    }
}

pub(crate) fn handle_build_accel_key(app: &mut App, key: KeyEvent) -> Result<Option<ExitAction>> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => Ok(Some(ExitAction::None)),
        KeyCode::Up | KeyCode::Char('k') => {
            if app.build_accel_menu_index > 0 {
                app.build_accel_menu_index -= 1;
            }
            Ok(None)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.build_accel_menu_index + 1 < app.build_accel_menu_len() {
                app.build_accel_menu_index += 1;
            }
            Ok(None)
        }
        KeyCode::Esc => {
            reset_to_menu(app);
            Ok(None)
        }
        KeyCode::Enter => {
            handle_build_accel_enter(app);
            Ok(None)
        }
        _ => Ok(None),
    }
}
