use std::sync::mpsc;

use crate::events::{UiMessage, WorkerEvent};

use super::{sanitize_log_line, send_log_line};

#[test]
fn sanitize_log_line_removes_carriage_returns() {
    // Terminal progress output can contain carriage returns that break log layout
    assert_eq!(sanitize_log_line("build\rline\r"), "buildline");
}

#[test]
fn send_log_line_delivers_worker_log_event() {
    let (tx, rx) = mpsc::sync_channel(1);

    send_log_line(&tx, "hello".to_string());

    let event = rx.try_recv().expect("log event");
    assert!(matches!(
        event,
        UiMessage::Worker(WorkerEvent::LogLine(message)) if message == "hello"
    ));
}
