//! Subprocess execution and log streaming helpers

use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError};
use std::thread;

use anyhow::{Context, Result};
use unixnotis_core::util;

use crate::app::events::{UiMessage, WorkerEvent};

use super::ActionContext;

// Track dropped log lines when the UI channel is saturated
// Avoids blocking log threads (stdout/stderr readers) while still surfacing
// loss to the UI once capacity returns, keeping the installer responsive
// under noisy subprocess output
static DROPPED_LOG_LINES: AtomicUsize = AtomicUsize::new(0);

// Eight thousand characters keep Cargo diagnostics useful while bounding TUI wrapping work
const MAX_INSTALLER_LOG_LINE_CHARS: usize = 8_192;
// Thirty-two KiB retains valid UTF-8 up to the display cap without buffering an unlimited line
const MAX_INSTALLER_LOG_LINE_BYTES: usize = 32_768;

pub fn run_command(
    ctx: &mut ActionContext,
    label: &str,
    mut command: Command,
    cwd: Option<&PathBuf>,
) -> Result<()> {
    run_command_with_output(ctx, label, &mut command, cwd, CommandOutput::StreamStdout)
}

pub fn run_command_without_stdout(
    ctx: &mut ActionContext,
    label: &str,
    mut command: Command,
    cwd: Option<&PathBuf>,
) -> Result<()> {
    run_command_with_output(ctx, label, &mut command, cwd, CommandOutput::SuppressStdout)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommandOutput {
    StreamStdout,
    SuppressStdout,
}

fn run_command_with_output(
    ctx: &mut ActionContext,
    label: &str,
    command: &mut Command,
    cwd: Option<&PathBuf>,
    output: CommandOutput,
) -> Result<()> {
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }

    // Some commands echo imported session variables on stdout
    // Keep those out of the TUI unless a caller explicitly needs command output
    let stdout = match output {
        CommandOutput::StreamStdout => Stdio::piped(),
        CommandOutput::SuppressStdout => Stdio::null(),
    };
    let mut child = command
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("command failed to start: {label}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let log_tx = ctx.log_tx.clone();
    let label_string = label.to_string();

    let stdout_handle = stdout.map(|stream| {
        let tx = log_tx.clone();
        let label = label_string.clone();
        thread::spawn(move || read_stream(stream, tx, label, "stdout"))
    });

    let stderr_handle = stderr.map(|stream| {
        let tx = log_tx.clone();
        let label = label_string.clone();
        thread::spawn(move || read_stream(stream, tx, label, "stderr"))
    });

    let status = child
        .wait()
        .with_context(|| format!("command failed to run: {label}"))?;

    // Surface log thread failures so command output issues are visible in the installer UI
    if let Some(handle) = stdout_handle {
        if let Err(err) = handle.join() {
            log_line(ctx, format!("Warning: stdout log thread panicked: {err:?}"));
        }
    }
    if let Some(handle) = stderr_handle {
        if let Err(err) = handle.join() {
            log_line(ctx, format!("Warning: stderr log thread panicked: {err:?}"));
        }
    }

    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("command failed: {label}"))
    }
}

pub fn log_line(ctx: &mut ActionContext, line: impl Into<String>) {
    send_log_line(&ctx.log_tx, line.into());
}

fn sanitize_log_line(line: &str) -> String {
    sanitize_log_line_with_source_truncation(line, false)
}

fn sanitize_log_line_with_source_truncation(line: &str, source_truncated: bool) -> String {
    // Reserve room for the truncation marker while stripping terminal and bidi controls
    let mut sanitized =
        util::sanitize_log_value(line, MAX_INSTALLER_LOG_LINE_CHARS.saturating_sub(3));
    if source_truncated && !sanitized.ends_with("...") {
        // A drained suffix still needs a visible marker when controls made the retained text short
        sanitized.push_str("...");
    }
    sanitized
}

fn read_stream(
    stream: impl std::io::Read,
    tx: SyncSender<UiMessage>,
    label: String,
    stream_name: &str,
) {
    let mut reader = BufReader::new(stream);
    // One reusable allocation bounds retained bytes across every physical input line
    let mut line = Vec::with_capacity(MAX_INSTALLER_LOG_LINE_BYTES);
    loop {
        match read_bounded_log_line(&mut reader, &mut line) {
            Ok(Some(source_truncated)) => {
                // Invalid subprocess bytes are replaced only after the retained input is bounded
                let line = String::from_utf8_lossy(&line);
                send_log_line_with_source_truncation(&tx, &line, source_truncated);
            }
            Ok(None) => break,
            Err(err) => {
                send_log_line(
                    &tx,
                    format!("Warning: log stream error for {label} ({stream_name}): {err}"),
                );
                break;
            }
        }
    }
}

fn read_bounded_log_line(
    reader: &mut impl BufRead,
    line: &mut Vec<u8>,
) -> io::Result<Option<bool>> {
    line.clear();
    let mut saw_input = false;
    let mut truncated = false;

    loop {
        // fill_buf exposes one bounded reader chunk without allocating for the full physical line
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            return if saw_input {
                Ok(Some(truncated))
            } else {
                Ok(None)
            };
        }

        let newline_offset = buffer.iter().position(|byte| *byte == b'\n');
        let content_len = newline_offset.unwrap_or(buffer.len());
        let remaining = MAX_INSTALLER_LOG_LINE_BYTES.saturating_sub(line.len());
        let copy_len = content_len.min(remaining);
        line.extend_from_slice(&buffer[..copy_len]);
        truncated |= copy_len < content_len;

        // Once the cap is reached later chunks are consumed without being copied
        let consumed = content_len + usize::from(newline_offset.is_some());
        let complete = newline_offset.is_some();
        reader.consume(consumed);
        saw_input = true;

        if complete {
            // Match BufRead::lines by removing the carriage return from a complete CRLF line
            if !truncated && line.last() == Some(&b'\r') {
                line.pop();
            }
            return Ok(Some(truncated));
        }
    }
}

fn send_log_line(tx: &SyncSender<UiMessage>, line: String) {
    let line = sanitize_log_line(&line);
    send_sanitized_log_line(tx, line);
}

fn send_log_line_with_source_truncation(
    tx: &SyncSender<UiMessage>,
    line: &str,
    source_truncated: bool,
) {
    // Every producer crosses one bounded, terminal-safe queue boundary
    let line = sanitize_log_line_with_source_truncation(line, source_truncated);
    send_sanitized_log_line(tx, line);
}

fn send_sanitized_log_line(tx: &SyncSender<UiMessage>, line: String) {
    // Non-blocking send keeps worker/log threads from stalling on a full UI queue
    // When the channel is full, the line is dropped and a summary warning is
    // emitted later once capacity frees up
    if try_send_log_line(tx, line) {
        flush_dropped_log_lines(tx);
    }
}

fn try_send_log_line(tx: &SyncSender<UiMessage>, line: String) -> bool {
    match tx.try_send(UiMessage::Worker(WorkerEvent::LogLine(line))) {
        Ok(()) => true,
        Err(TrySendError::Full(_)) => {
            // Count dropped lines so the UI can be told once capacity returns
            DROPPED_LOG_LINES.fetch_add(1, Ordering::Relaxed);
            false
        }
        Err(TrySendError::Disconnected(_)) => false,
    }
}

fn flush_dropped_log_lines(tx: &SyncSender<UiMessage>) {
    let dropped = DROPPED_LOG_LINES.swap(0, Ordering::Relaxed);
    if dropped == 0 {
        return;
    }
    let message = format!("Warning: {dropped} log line(s) dropped because the UI was busy");
    // If the UI channel is still full, retain the count for a future flush
    if let Err(err) = tx.try_send(UiMessage::Worker(WorkerEvent::LogLine(message))) {
        if matches!(err, TrySendError::Full(_)) {
            // Restore the dropped count so the warning is emitted later instead
            // of being lost under sustained saturation
            DROPPED_LOG_LINES.fetch_add(dropped, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
#[path = "tests/process.rs"]
mod tests;
