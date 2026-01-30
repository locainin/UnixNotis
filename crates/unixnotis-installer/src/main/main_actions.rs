//! Action execution, worker coordination, and state transitions for the installer.

use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::actions::{
    build_plan, check_install_state, detect_build_accel, detect_build_accel_without_repo, run_step,
    steps_from_plan, write_build_accel_config, ActionContext, BuildAccelOutcome, StepKind,
};
use crate::app::{App, ProgressState, Screen};
use crate::events::{UiMessage, WorkerEvent};
use crate::model::{ActionMode, StepStatus};
use crate::paths::InstallPaths;
use crate::terminal::TerminalGuard;
use crate::ui;

pub(crate) fn start_action(
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

    let (plan, restore_backup) = match mode {
        ActionMode::Reset => match &app.reset_action {
            crate::model::ResetAction::ResetDefaults => (build_plan(mode, app.verify), None),
            crate::model::ResetAction::RestoreBackup { path } => {
                (vec![StepKind::RestoreConfig], Some(path.clone()))
            }
        },
        _ => (build_plan(mode, app.verify), None),
    };

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
        run_action_worker(
            plan,
            mode,
            detection,
            paths,
            install_state,
            restore_backup,
            ui_tx,
        );
    });

    Ok(())
}

fn run_action_worker(
    plan: Vec<StepKind>,
    mode: ActionMode,
    detection: crate::detect::Detection,
    paths: InstallPaths,
    install_state: Option<crate::actions::InstallState>,
    restore_backup: Option<PathBuf>,
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
                restore_backup: restore_backup.clone(),
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

pub(crate) fn apply_worker_event(app: &mut App, event: WorkerEvent) {
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
            app.progress_ready_at = Some(std::time::Instant::now() + Duration::from_millis(400));
        }
        WorkerEvent::LogLine(line) => {
            append_log(app, line);
        }
        WorkerEvent::Finished => {
            if matches!(app.progress_state, ProgressState::Running) {
                app.progress_state = ProgressState::Completed;
                app.progress_ready_at =
                    Some(std::time::Instant::now() + Duration::from_millis(400));
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

pub(crate) fn reset_to_menu(app: &mut App) {
    app.screen = Screen::Welcome;
    app.last_error = None;
    app.logs.clear();
    app.steps.clear();
    app.progress_state = ProgressState::Idle;
    app.progress_ready_at = None;
    app.build_accel = None;
    app.build_accel_menu_index = 0;
    app.reset_menu_index = 0;
    app.reset_action = crate::model::ResetAction::ResetDefaults;
    app.restore_backups.clear();
    app.restore_menu_index = 0;
    app.refresh();
}

pub(crate) fn prepare_build_accel_prompt(app: &mut App) {
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

pub(crate) fn handle_build_accel_enter(app: &mut App) {
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

pub(crate) fn run_trial(repo_root: PathBuf) -> Result<()> {
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
