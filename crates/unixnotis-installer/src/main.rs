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
use std::time::{Duration, Instant};

use crate::actions::{
    build_plan, check_install_state, detect_build_accel, detect_build_accel_without_repo, run_step,
    steps_from_plan, write_build_accel_config, ActionContext, BuildAccelOutcome, StepKind,
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
            Screen::Progress(_) => handle_progress_key(app, key),
            Screen::BuildAccel => handle_build_accel_key(app, key),
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

fn handle_progress_key(app: &mut App, key: KeyEvent) -> Result<Option<ExitAction>> {
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

fn handle_build_accel_key(app: &mut App, key: KeyEvent) -> Result<Option<ExitAction>> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => Ok(Some(ExitAction::None)),
        KeyCode::Up => {
            if app.build_accel_menu_index > 0 {
                app.build_accel_menu_index -= 1;
            }
            Ok(None)
        }
        KeyCode::Down => {
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

fn start_action(
    app: &mut App,
    terminal_guard: &mut TerminalGuard,
    ui_tx: &mpsc::SyncSender<UiMessage>,
    mode: ActionMode,
) -> Result<()> {
    let paths = InstallPaths::discover()?;
    let install_state = if mode == ActionMode::Install {
        Some(check_install_state(&paths))
    } else {
        None
    };

    let plan = build_plan(mode, app.verify);

    app.steps = steps_from_plan(&plan);
    app.logs.clear();
    app.last_error = None;
    app.progress_state = ProgressState::Running;
    app.progress_ready_at = None;
    app.screen = Screen::Progress(mode);

    terminal_guard
        .terminal_mut()
        .draw(|frame| ui::draw(frame, app))?;

    let detection = app.detection.clone();
    let ui_tx = ui_tx.clone();
    thread::spawn(move || {
        run_action_worker(plan, mode, detection, paths, install_state, ui_tx);
    });

    Ok(())
}

fn run_action_worker(
    plan: Vec<StepKind>,
    mode: ActionMode,
    detection: crate::detect::Detection,
    paths: InstallPaths,
    install_state: Option<crate::actions::InstallState>,
    ui_tx: mpsc::SyncSender<UiMessage>,
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
                action_mode: mode,
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
            app.progress_ready_at = Some(Instant::now() + Duration::from_millis(400));
        }
        WorkerEvent::LogLine(line) => {
            append_log(app, line);
        }
        WorkerEvent::Finished => {
            if matches!(app.progress_state, ProgressState::Running) {
                app.progress_state = ProgressState::Completed;
                app.progress_ready_at = Some(Instant::now() + Duration::from_millis(400));
            }
        }
    }
}

fn append_log(app: &mut App, line: String) {
    // Bound log memory usage by trimming old entries.
    const MAX_LINES: usize = 200;

    app.logs.push_back(line);

    if app.logs.len() > MAX_LINES {
        // VecDeque allows O(1) removal from the front.
        while app.logs.len() > MAX_LINES {
            app.logs.pop_front();
        }
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

fn reset_to_menu(app: &mut App) {
    app.screen = Screen::Welcome;
    app.last_error = None;
    app.logs.clear();
    app.steps.clear();
    app.progress_state = ProgressState::Idle;
    app.progress_ready_at = None;
    app.build_accel = None;
    app.build_accel_menu_index = 0;
    app.refresh();
}

fn prepare_build_accel_prompt(app: &mut App) {
    // Snapshot detection so the prompt remains stable while the user decides.
    let detection = match InstallPaths::discover() {
        Ok(paths) => detect_build_accel(&paths.repo_root),
        Err(err) => detect_build_accel_without_repo(err.to_string()),
    };
    app.build_accel = Some(crate::app::BuildAccelState {
        detection,
        outcome: None,
    });
    app.build_accel_menu_index = 0;
}

fn apply_build_accel_setup(app: &mut App) {
    // Writes per-repository Cargo config only when explicitly requested.
    let Some(state) = app.build_accel.as_mut() else {
        return;
    };
    let paths = match InstallPaths::discover() {
        Ok(paths) => paths,
        Err(err) => {
            state.outcome = Some(BuildAccelOutcome::Failed(err.to_string()));
            return;
        }
    };
    let outcome = write_build_accel_config(&paths.repo_root, &state.detection);
    state.outcome = Some(outcome);
    // Keep selection on the only available action once a result is shown.
    app.build_accel_menu_index = 0;
    // Refresh detection so config state is reflected in the prompt immediately.
    state.detection = detect_build_accel(&paths.repo_root);
}

fn handle_build_accel_enter(app: &mut App) {
    match app.build_accel_menu_mode() {
        crate::app::BuildAccelMenuMode::ReturnOnly => {
            reset_to_menu(app);
        }
        crate::app::BuildAccelMenuMode::EnableOrSkip => {
            if app.build_accel_menu_index == 0 {
                apply_build_accel_setup(app);
            } else {
                reset_to_menu(app);
            }
        }
        crate::app::BuildAccelMenuMode::Reinstall => {
            if app.build_accel_menu_index == 0 {
                reset_to_menu(app);
            } else {
                apply_build_accel_setup(app);
            }
        }
    }
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
