use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::app::events::{ExitAction, UiMessage, WorkerEvent};

#[test]
fn ui_message_can_carry_keyboard_input() {
    let event = Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
    let message = UiMessage::Input(event);

    // Input events stay separate from worker progress so the UI loop can dispatch cleanly
    match message {
        UiMessage::Input(Event::Key(key)) => assert_eq!(key.code, KeyCode::Char('q')),
        _ => panic!("expected key input"),
    }
}

#[test]
fn ui_message_can_carry_release_status_update() {
    let status = crate::release::ReleaseStatus::current_only();
    let message = UiMessage::ReleaseStatus(status);

    // Release checks arrive separately so startup can draw before network work finishes
    match message {
        UiMessage::ReleaseStatus(status) => assert_eq!(status.latest, None),
        _ => panic!("expected release status"),
    }
}

#[test]
fn worker_event_failed_keeps_step_index_and_message() {
    let event = WorkerEvent::StepFailed {
        index: 3,
        summary: "service start failed".to_string(),
        detail: "service start failed: bus unavailable".to_string(),
    };

    // Failure events keep a short status summary and a full diagnostic chain
    match event {
        WorkerEvent::StepFailed {
            index,
            summary,
            detail,
        } => {
            assert_eq!(index, 3);
            assert_eq!(summary, "service start failed");
            assert_eq!(detail, "service start failed: bus unavailable");
        }
        _ => panic!("expected failed event"),
    }
}

#[test]
fn worker_event_recovery_required_keeps_detailed_failure_without_finished_event() {
    let event = WorkerEvent::RecoveryRequired {
        index: 2,
        summary: "rollback state is unknown".to_string(),
        detail: "rollback state is unknown: journal unreadable".to_string(),
    };

    match event {
        WorkerEvent::RecoveryRequired {
            index,
            summary,
            detail,
        } => {
            assert_eq!(index, 2);
            assert_eq!(summary, "rollback state is unknown");
            assert_eq!(detail, "rollback state is unknown: journal unreadable");
        }
        _ => panic!("expected recovery-required event"),
    }
}

#[test]
fn worker_log_line_keeps_original_text() {
    let event = WorkerEvent::LogLine("Installed service artifact".to_string());

    // Log messages are already sanitized before display, so the event should not rewrite them
    match event {
        WorkerEvent::LogLine(message) => assert_eq!(message, "Installed service artifact"),
        _ => panic!("expected log line"),
    }
}

#[test]
fn trial_exit_action_keeps_selected_repository_path() {
    let action = ExitAction::RunTrial {
        repo_root: "workspace/UnixNotis".into(),
    };

    match action {
        ExitAction::RunTrial { repo_root } => {
            assert_eq!(repo_root, std::path::Path::new("workspace/UnixNotis"));
        }
        ExitAction::None => panic!("expected trial action"),
    }
}
