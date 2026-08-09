use crate::app::events::WorkerEvent;
use crate::app::{BuildAccelState, ProgressState, Screen};
use crate::model::{ActionStep, ResetAction, StepStatus};

use super::super::events::{apply_worker_event, reset_to_menu};
use super::support::app_with_steps;

#[test]
fn worker_step_events_update_only_existing_steps() {
    let mut app = app_with_steps();

    apply_worker_event(&mut app, WorkerEvent::StepStarted(0));
    apply_worker_event(&mut app, WorkerEvent::StepCompleted(1));
    apply_worker_event(&mut app, WorkerEvent::StepStarted(99));

    // Out-of-range events can arrive after UI reset and should be ignored
    assert_eq!(app.steps[0].status, StepStatus::Running);
    assert_eq!(app.steps[1].status, StepStatus::Done);
    assert_eq!(app.progress_state, ProgressState::Running);
}

#[test]
fn worker_failure_marks_step_logs_error_and_blocks_finished_from_success() {
    let mut app = app_with_steps();

    apply_worker_event(
        &mut app,
        WorkerEvent::StepFailed {
            index: 1,
            summary: "boom".to_string(),
            detail: "boom: nested cause".to_string(),
        },
    );
    apply_worker_event(&mut app, WorkerEvent::Finished);

    // Finished must not erase the failure state produced by the worker
    assert_eq!(app.steps[1].status, StepStatus::Failed);
    assert_eq!(app.progress_state, ProgressState::Failed);
    assert_eq!(app.last_error.as_deref(), Some("boom"));
    assert_eq!(
        app.logs.back().map(String::as_str),
        Some("Error: boom: nested cause")
    );
    assert!(app.progress_ready_at.is_some());
    assert!(app
        .progress_ready_at
        .is_some_and(|deadline| deadline > std::time::Instant::now()));
}

#[test]
fn recovery_required_event_keeps_the_worker_state_inhibited() {
    let mut app = app_with_steps();

    apply_worker_event(
        &mut app,
        WorkerEvent::RecoveryRequired {
            index: 1,
            summary: "rollback failed".to_string(),
            detail: "rollback failed: service state unknown".to_string(),
        },
    );
    apply_worker_event(&mut app, WorkerEvent::Finished);

    // A catastrophic worker intentionally does not finish, so Finished cannot turn this into success
    assert_eq!(app.steps[1].status, StepStatus::Failed);
    assert_eq!(app.progress_state, ProgressState::RecoveryRequired);
    assert_eq!(app.last_error.as_deref(), Some("rollback failed"));
    assert_eq!(
        app.logs.back().map(String::as_str),
        Some("CRITICAL: daemon activation remains inhibited because safe rollback could not be proven.")
    );
    assert!(app.progress_ready_at.is_none());
}

#[test]
fn worker_finished_marks_running_action_completed() {
    let mut app = app_with_steps();

    apply_worker_event(&mut app, WorkerEvent::Finished);

    // Successful workers delay navigation briefly so users can read completion state
    assert_eq!(app.progress_state, ProgressState::Completed);
    assert!(app.progress_ready_at.is_some());
    assert!(app
        .progress_ready_at
        .is_some_and(|deadline| deadline > std::time::Instant::now()));
}

#[test]
fn worker_logs_keep_recent_two_hundred_entries() {
    let mut app = app_with_steps();

    for index in 0..250 {
        apply_worker_event(&mut app, WorkerEvent::LogLine(format!("line-{index}")));
    }

    // Progress logs are bounded to prevent noisy commands from growing memory forever
    assert_eq!(app.logs.len(), 200);
    assert_eq!(app.logs.front().map(String::as_str), Some("line-50"));
    assert_eq!(app.logs.back().map(String::as_str), Some("line-249"));
}

#[test]
fn reset_to_menu_clears_transient_action_state() {
    let _lock = crate::test_support::env::test_env_lock();
    let mut app = crate::app::App::new(None);
    app.steps = vec![ActionStep {
        name: "first",
        status: StepStatus::Running,
    }];
    app.screen = Screen::BuildAccel;
    app.logs.push_back("old log".to_string());
    app.last_error = Some("old error".to_string());
    app.progress_state = ProgressState::Failed;
    app.progress_ready_at = Some(std::time::Instant::now());
    app.build_accel = Some(BuildAccelState {
        detection: crate::actions::BuildAccelDetection {
            sccache_installed: true,
            mold_installed: false,
            config_status: crate::actions::BuildAccelConfigStatus::Missing,
        },
        outcome: None,
    });
    app.build_accel_menu_index = 9;
    app.reset_menu_index = 2;
    app.reset_action = ResetAction::RestoreBackup {
        path: "old-backup".into(),
    };
    app.restore_backups = vec!["old-backup".into()];
    app.restore_menu_index = 1;

    reset_to_menu(&mut app);

    // Returning to the menu should discard stale progress, restore, and build prompt state
    assert_eq!(app.screen, Screen::Welcome);
    assert_eq!(app.progress_state, ProgressState::Idle);
    assert!(app.logs.is_empty());
    assert!(app.steps.is_empty());
    assert!(app.last_error.is_none());
    assert!(app.progress_ready_at.is_none());
    assert!(app.build_accel.is_none());
    assert_eq!(app.build_accel_menu_index, 0);
    assert_eq!(app.reset_menu_index, 0);
    assert!(matches!(app.reset_action, ResetAction::ResetDefaults));
    assert!(app.restore_backups.is_empty());
    assert_eq!(app.restore_menu_index, 0);
}
