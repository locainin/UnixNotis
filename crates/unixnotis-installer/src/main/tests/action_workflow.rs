use crate::action_workflow::apply_worker_event;
use crate::app::{App, ProgressState};
use crate::events::WorkerEvent;
use crate::model::{ActionStep, StepStatus};

fn app_with_steps() -> App {
    let _lock = crate::tests::env::test_env_lock();
    let mut app = App::new(None);
    app.steps = vec![
        ActionStep {
            name: "first",
            status: StepStatus::Pending,
        },
        ActionStep {
            name: "second",
            status: StepStatus::Pending,
        },
    ];
    app.progress_state = ProgressState::Running;
    app
}

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

    apply_worker_event(&mut app, WorkerEvent::StepFailed(1, "boom".to_string()));
    apply_worker_event(&mut app, WorkerEvent::Finished);

    // Finished must not erase the failure state produced by the worker
    assert_eq!(app.steps[1].status, StepStatus::Failed);
    assert_eq!(app.progress_state, ProgressState::Failed);
    assert_eq!(app.last_error.as_deref(), Some("boom"));
    assert_eq!(app.logs.back().map(String::as_str), Some("Error: boom"));
    assert!(app.progress_ready_at.is_some());
}

#[test]
fn worker_finished_marks_running_action_completed() {
    let mut app = app_with_steps();

    apply_worker_event(&mut app, WorkerEvent::Finished);

    // Successful workers delay navigation briefly so users can read completion state
    assert_eq!(app.progress_state, ProgressState::Completed);
    assert!(app.progress_ready_at.is_some());
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
