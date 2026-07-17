use std::io::{BufReader, Cursor};
use std::sync::atomic::Ordering;
use std::sync::mpsc;

use crate::app::events::{UiMessage, WorkerEvent};

use super::{
    flush_dropped_log_lines, read_bounded_log_line, sanitize_log_line,
    sanitize_log_line_with_source_truncation, send_log_line, DROPPED_LOG_LINES,
    MAX_INSTALLER_LOG_LINE_BYTES, MAX_INSTALLER_LOG_LINE_CHARS,
};

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
fn sanitize_log_line_marks_a_drained_control_only_suffix() {
    let sanitized = sanitize_log_line_with_source_truncation("\u{1b}\u{7}", true);

    assert_eq!(sanitized, "...");
}

#[test]
fn bounded_log_reader_drains_an_oversized_line_before_reading_the_next_line() {
    let mut input = vec![b'x'; MAX_INSTALLER_LOG_LINE_BYTES * 4];
    input.extend_from_slice(b"\ntail\r\n");
    let mut reader = BufReader::with_capacity(1024, Cursor::new(input));
    let mut line = Vec::with_capacity(MAX_INSTALLER_LOG_LINE_BYTES);

    let first_truncated = read_bounded_log_line(&mut reader, &mut line)
        .expect("read oversized line")
        .expect("oversized line");
    assert!(first_truncated);
    assert_eq!(line.len(), MAX_INSTALLER_LOG_LINE_BYTES);

    let tail_truncated = read_bounded_log_line(&mut reader, &mut line)
        .expect("read following line")
        .expect("following line");
    assert!(!tail_truncated);
    assert_eq!(line, b"tail");
    assert_eq!(
        read_bounded_log_line(&mut reader, &mut line).expect("read end of stream"),
        None
    );
}

#[test]
fn bounded_log_reader_caps_an_oversized_line_without_a_newline() {
    let input = vec![b'x'; MAX_INSTALLER_LOG_LINE_BYTES * 4];
    let mut reader = BufReader::with_capacity(1024, Cursor::new(input));
    let mut line = Vec::with_capacity(MAX_INSTALLER_LOG_LINE_BYTES);

    let truncated = read_bounded_log_line(&mut reader, &mut line)
        .expect("read unterminated line")
        .expect("unterminated line");

    assert!(truncated);
    assert_eq!(line.len(), MAX_INSTALLER_LOG_LINE_BYTES);
    assert_eq!(
        read_bounded_log_line(&mut reader, &mut line).expect("read end of stream"),
        None
    );
}

#[test]
fn bounded_log_reader_consumes_the_newline_before_the_next_read() {
    let mut reader = BufReader::new(Cursor::new(b"line\n"));
    let mut line = Vec::with_capacity(MAX_INSTALLER_LOG_LINE_BYTES);

    let truncated = read_bounded_log_line(&mut reader, &mut line)
        .expect("read complete line")
        .expect("complete line");

    assert!(!truncated);
    assert_eq!(line, b"line");
    assert_eq!(
        read_bounded_log_line(&mut reader, &mut line).expect("read end of stream"),
        None
    );
}

#[test]
fn bounded_log_reader_preserves_retained_carriage_return_when_suffix_is_truncated() {
    let mut input = vec![b'x'; MAX_INSTALLER_LOG_LINE_BYTES - 1];
    input.extend_from_slice(b"\rdiscarded\n");
    let mut reader = BufReader::new(Cursor::new(input));
    let mut line = Vec::with_capacity(MAX_INSTALLER_LOG_LINE_BYTES);

    let truncated = read_bounded_log_line(&mut reader, &mut line)
        .expect("read oversized carriage-return line")
        .expect("oversized carriage-return line");

    assert!(truncated);
    assert_eq!(line.len(), MAX_INSTALLER_LOG_LINE_BYTES);
    assert_eq!(line.last(), Some(&b'\r'));
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

#[test]
fn flush_dropped_log_lines_emits_and_clears_the_retained_count() {
    let (tx, rx) = mpsc::sync_channel(1);
    DROPPED_LOG_LINES.store(3, Ordering::Relaxed);

    flush_dropped_log_lines(&tx);

    let event = rx.try_recv().expect("dropped-line summary");
    assert!(matches!(
        event,
        UiMessage::Worker(WorkerEvent::LogLine(message))
            if message == "Warning: 3 log line(s) dropped because the UI was busy"
    ));
    assert_eq!(DROPPED_LOG_LINES.load(Ordering::Relaxed), 0);
}
