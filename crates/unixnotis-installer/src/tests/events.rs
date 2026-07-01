use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::events::{UiMessage, WorkerEvent};

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
fn worker_event_failed_keeps_step_index_and_message() {
    let event = WorkerEvent::StepFailed(3, "service start failed".to_string());

    // Failure events need both fields for progress rendering and final error text
    match event {
        WorkerEvent::StepFailed(index, message) => {
            assert_eq!(index, 3);
            assert_eq!(message, "service start failed");
        }
        _ => panic!("expected failed event"),
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
