//! UI-facing workflow state transitions

use std::time::Duration;

use crate::app::events::WorkerEvent;
use crate::app::{App, ProgressState, Screen};
use crate::model::StepStatus;

pub fn apply_worker_event(app: &mut App, event: WorkerEvent) {
    match event {
        WorkerEvent::StepStarted(index) => {
            // Missing indices are ignored because UI state may have reset after worker start
            if let Some(step) = app.steps.get_mut(index) {
                step.status = StepStatus::Running;
            }
        }
        WorkerEvent::StepCompleted(index) => {
            // Step completion is best-effort because the worker is decoupled from UI state
            if let Some(step) = app.steps.get_mut(index) {
                step.status = StepStatus::Done;
            }
        }
        WorkerEvent::StepFailed {
            index,
            summary,
            detail,
        } => {
            // Preserve the error message for the progress screen
            if let Some(step) = app.steps.get_mut(index) {
                step.status = StepStatus::Failed;
            }
            app.last_error = Some(summary);
            // Keep the compact summary in the status panel and the complete anyhow chain in logs
            append_log(app, format!("Error: {detail}"));
            app.progress_state = ProgressState::Failed;
            app.progress_ready_at = Some(std::time::Instant::now() + Duration::from_millis(400));
        }
        WorkerEvent::RecoveryRequired {
            index,
            summary,
            detail,
        } => {
            // A recovery-required worker is still alive and still owns activation
            if let Some(step) = app.steps.get_mut(index) {
                step.status = StepStatus::Failed;
            }
            app.last_error = Some(summary);
            append_log(app, format!("Error: {detail}"));
            append_log(
                app,
                "CRITICAL: daemon activation remains inhibited because safe rollback could not be proven."
                    .to_string(),
            );
            app.progress_state = ProgressState::RecoveryRequired;
            app.progress_ready_at = None;
        }
        WorkerEvent::LogLine(line) => {
            // Worker logs are bounded by append_log
            append_log(app, line);
        }
        WorkerEvent::Finished => {
            // Finished should not overwrite a failed progress state
            if matches!(app.progress_state, ProgressState::Running) {
                app.progress_state = ProgressState::Completed;
                app.progress_ready_at =
                    Some(std::time::Instant::now() + Duration::from_millis(400));
            }
        }
    }
}

fn append_log(app: &mut App, line: String) {
    // Bound log memory usage by trimming old entries
    const MAX_LINES: usize = 200;

    app.logs.push_back(line);

    // Each call adds one row, so at most one old row needs removal
    if app.logs.len() > MAX_LINES {
        let _oldest = app.logs.pop_front();
    }
}

pub fn reset_to_menu(app: &mut App) {
    // Return every transient menu and progress field to the welcome state
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
