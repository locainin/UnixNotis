//! UnixNotis installer entrypoint with a ratatui-driven flow.

mod actions;
mod app;
mod checks;
mod detect;
mod events;
mod model;
mod paths;
mod terminal;
mod ui;

use anyhow::{anyhow, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use crate::actions::{
    build_plan, check_install_state, run_step, steps_from_plan, ActionContext, StepKind,
};
use crate::app::{App, MenuItem, ProgressState, Screen};
use crate::events::{UiMessage, WorkerEvent};
use crate::model::{ActionMode, StepStatus};
use crate::paths::InstallPaths;
use crate::terminal::TerminalGuard;

fn main() -> Result<()> {
    let mut app = App::new();
    let mut terminal_guard = TerminalGuard::new()?;
    let exit_action = run_app(&mut terminal_guard, &mut app);
    terminal_guard.restore()?;

    match exit_action {
        Ok(ExitAction::None) => Ok(()),
        Ok(ExitAction::RunTrial { repo_root }) => run_trial(repo_root),
        Err(err) => Err(err),
    }
}

enum ExitAction {
    None,
    RunTrial { repo_root: PathBuf },
}

fn run_app(terminal_guard: &mut TerminalGuard, app: &mut App) -> Result<ExitAction> {
    let (ui_tx, ui_rx) = mpsc::channel::<UiMessage>();
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
    ui_tx: &mpsc::Sender<UiMessage>,
    event: Event,
) -> Result<Option<ExitAction>> {
    match event {
        Event::Key(key) => match app.screen {
            Screen::Welcome => handle_welcome_key(app, key),
            Screen::Confirm(mode) => handle_confirm_key(app, terminal_guard, ui_tx, key, mode),
            Screen::Progress(_) => handle_progress_key(app, key),
        },
        Event::Resize(_, _) => Ok(None),
        _ => Ok(None),
    }
}

fn handle_welcome_key(app: &mut App, key: KeyEvent) -> Result<Option<ExitAction>> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => Ok(Some(ExitAction::None)),
        KeyCode::Up => {
            if app.menu_index > 0 {
                app.menu_index -= 1;
            }
            Ok(None)
        }
        KeyCode::Down => {
            if app.menu_index + 1 < App::menu_items().len() {
                app.menu_index += 1;
            }
            Ok(None)
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            app.refresh();
            Ok(None)
        }
        KeyCode::Char('v') | KeyCode::Char('V') => {
            app.verify = !app.verify;
            Ok(None)
        }
        KeyCode::Enter => match app.selected_menu() {
            MenuItem::Quit => Ok(Some(ExitAction::None)),
            MenuItem::Action(mode) => {
                app.screen = Screen::Confirm(mode);
                Ok(None)
            }
        },
        _ => Ok(None),
    }
}

fn handle_confirm_key(
    app: &mut App,
    terminal_guard: &mut TerminalGuard,
    ui_tx: &mpsc::Sender<UiMessage>,
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
                ActionMode::Install | ActionMode::Uninstall => {
                    start_action(app, terminal_guard, ui_tx, mode)?;
                }
            }

            Ok(None)
        }
        _ => Ok(None),
    }
}

fn handle_progress_key(app: &mut App, key: KeyEvent) -> Result<Option<ExitAction>> {
    if matches!(app.progress_state, ProgressState::Running) {
        return Ok(None);
    }
    match key.code {
        KeyCode::Enter => {
            reset_to_menu(app);
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

fn start_action(
    app: &mut App,
    terminal_guard: &mut TerminalGuard,
    ui_tx: &mpsc::Sender<UiMessage>,
    mode: ActionMode,
) -> Result<()> {
    let paths = InstallPaths::discover()?;
    let install_state = if mode == ActionMode::Install {
        Some(check_install_state(&paths))
    } else {
        None
    };

    let plan = if let Some(state) = install_state.as_ref() {
        if state.is_fully_installed() {
            vec![StepKind::InstallCheck]
        } else {
            build_plan(mode, app.verify)
        }
    } else {
        build_plan(mode, app.verify)
    };

    app.steps = steps_from_plan(&plan);
    app.logs.clear();
    app.last_error = None;
    app.progress_state = ProgressState::Running;
    app.screen = Screen::Progress(mode);

    terminal_guard
        .terminal_mut()
        .draw(|frame| ui::draw(frame, app))?;

    let detection = app.detection.clone();
    let ui_tx = ui_tx.clone();
    thread::spawn(move || {
        run_action_worker(plan, detection, paths, install_state, ui_tx);
    });

    Ok(())
}

fn run_action_worker(
    plan: Vec<StepKind>,
    detection: crate::detect::Detection,
    paths: InstallPaths,
    install_state: Option<crate::actions::InstallState>,
    ui_tx: mpsc::Sender<UiMessage>,
) {
    // Run plan steps on the worker thread and stream progress events to the UI.
    for (index, step) in plan.iter().enumerate() {
        // Index maps to app.steps in the UI state.
        let _ = ui_tx.send(UiMessage::Worker(WorkerEvent::StepStarted(index)));

        // Build per-step context; clone install_state to avoid borrow issues.
        let result = {
            let mut ctx = ActionContext {
                detection: &detection,
                paths: &paths,
                install_state: install_state.clone(),
                log_tx: ui_tx.clone(),
            };
            run_step(*step, &mut ctx)
        };

        match result {
            Ok(()) => {
                let _ = ui_tx.send(UiMessage::Worker(WorkerEvent::StepCompleted(index)));
            }
            Err(err) => {
                let _ = ui_tx.send(UiMessage::Worker(WorkerEvent::StepFailed(
                    index,
                    err.to_string(),
                )));
                let _ = ui_tx.send(UiMessage::Worker(WorkerEvent::Finished));
                return;
            }
        }
    }

    let _ = ui_tx.send(UiMessage::Worker(WorkerEvent::Finished));
}

fn apply_worker_event(app: &mut App, event: WorkerEvent) {
    match event {
        WorkerEvent::StepStarted(index) => {
            if let Some(step) = app.steps.get_mut(index) {
                step.status = StepStatus::Running;
            }
        }
        WorkerEvent::StepCompleted(index) => {
            if let Some(step) = app.steps.get_mut(index) {
                step.status = StepStatus::Done;
            }
        }
        WorkerEvent::StepFailed(index, err) => {
            if let Some(step) = app.steps.get_mut(index) {
                step.status = StepStatus::Failed;
            }
            app.last_error = Some(err.clone());
            append_log(app, format!("Error: {}", err));
            app.progress_state = ProgressState::Failed;
        }
        WorkerEvent::LogLine(line) => {
            append_log(app, line);
        }
        WorkerEvent::Finished => {
            if matches!(app.progress_state, ProgressState::Running) {
                app.progress_state = ProgressState::Completed;
            }
        }
    }
}

fn append_log(app: &mut App, line: String) {
    // Bound log memory usage by trimming old entries.
    const MAX_LINES: usize = 200;

    app.logs.push(line);

    if app.logs.len() > MAX_LINES {
        let excess = app.logs.len() - MAX_LINES;
        app.logs.drain(0..excess);
    }
}

fn spawn_input_thread(ui_tx: mpsc::Sender<UiMessage>) {
    // Forward blocking terminal events to the UI thread; exit on channel close.
    thread::spawn(move || {
        while let Ok(event) = event::read() {
            if ui_tx.send(UiMessage::Input(event)).is_err() {
                break;
            }
        }
    });
}


fn reset_to_menu(app: &mut App) {
    app.screen = Screen::Welcome;
    app.last_error = None;
    app.logs.clear();
    app.steps.clear();
    app.progress_state = ProgressState::Idle;
    app.refresh();
}

fn run_trial(repo_root: PathBuf) -> Result<()> {
    println!("Starting UnixNotis trial run.");
    println!("Press Ctrl+C to stop and restore the previous daemon.");

    let status = std::process::Command::new("cargo")
        .args([
            "run",
            "--release",
            "-p",
            "unixnotis-daemon",
            "--",
            "--trial",
            "--restore",
            "auto",
            "--yes",
        ])
        .current_dir(&repo_root)
        .status()
        .map_err(|err| anyhow!("failed to run trial: {}", err))?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("trial run exited with failure"))
    }
}
