use std::sync::mpsc;

use crate::app::events::{UiMessage, WorkerEvent};

use super::{sanitize_log_line, send_log_line, MAX_INSTALLER_LOG_LINE_CHARS};

#[test]
fn sanitize_log_line_flattens_terminal_controls() {
    let sanitized = sanitize_log_line("build\r\u{1b}[31mred\u{7}\u{202e}line");

    assert_eq!(sanitized, "build  [31mred line");
    assert!(!sanitized.chars().any(char::is_control));
    assert!(!sanitized.contains('\u{202e}'));
}

#[test]
fn sanitize_log_line_caps_pathological_subprocess_output() {
    let oversized = "x".repeat(MAX_INSTALLER_LOG_LINE_CHARS * 2);

    let sanitized = sanitize_log_line(&oversized);

    assert_eq!(sanitized.chars().count(), MAX_INSTALLER_LOG_LINE_CHARS);
    assert!(sanitized.ends_with("..."));
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

#[test]
fn send_log_line_sanitizes_before_queueing() {
    let (tx, rx) = mpsc::sync_channel(1);

    send_log_line(&tx, "unsafe\u{1b}[2Jline".to_string());

    let event = rx.try_recv().expect("log event");
    assert!(matches!(
        event,
        UiMessage::Worker(WorkerEvent::LogLine(message)) if message == "unsafe [2Jline"
    ));
}
