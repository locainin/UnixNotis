//! Subprocess execution and log streaming helpers.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError};
use std::thread;

use anyhow::{Context, Result};

use crate::events::{UiMessage, WorkerEvent};

use super::ActionContext;

// Track dropped log lines when the UI channel is saturated.
// Avoids blocking log threads (stdout/stderr readers) while still surfacing
// loss to the UI once capacity returns, keeping the installer responsive
// under noisy subprocess output.
static DROPPED_LOG_LINES: AtomicUsize = AtomicUsize::new(0);

pub fn run_command(
    ctx: &mut ActionContext,
    label: &str,
    mut command: Command,
    cwd: Option<&PathBuf>,
) -> Result<()> {
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }

    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("command failed to start: {}", label))?;

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
        .with_context(|| format!("command failed to run: {}", label))?;

    // Surface log thread failures so command output issues are visible in the installer UI.
    if let Some(handle) = stdout_handle {
        if let Err(err) = handle.join() {
            log_line(
                ctx,
                format!("Warning: stdout log thread panicked: {:?}", err),
            );
        }
    }
    if let Some(handle) = stderr_handle {
        if let Err(err) = handle.join() {
            log_line(
                ctx,
                format!("Warning: stderr log thread panicked: {:?}", err),
            );
        }
    }

    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("command failed: {}", label))
    }
}

pub fn log_line(ctx: &mut ActionContext, line: impl Into<String>) {
    send_log_line(&ctx.log_tx, line.into());
}

fn sanitize_log_line(line: &str) -> String {
    line.replace('\r', "")
}

fn read_stream(
    stream: impl std::io::Read,
    tx: SyncSender<UiMessage>,
    label: String,
    stream_name: &str,
) {
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        match line {
            Ok(line) => {
                send_log_line(&tx, sanitize_log_line(&line));
            }
            Err(err) => {
                send_log_line(
                    &tx,
                    format!(
                        "Warning: log stream error for {} ({}): {}",
                        label, stream_name, err
                    ),
                );
                break;
            }
        }
    }
}

fn send_log_line(tx: &SyncSender<UiMessage>, line: String) {
    // Non-blocking send keeps worker/log threads from stalling on a full UI queue.
    // When the channel is full, the line is dropped and a summary warning is
    // emitted later once capacity frees up.
    if try_send_log_line(tx, line) {
        flush_dropped_log_lines(tx);
    }
}

fn try_send_log_line(tx: &SyncSender<UiMessage>, line: String) -> bool {
    match tx.try_send(UiMessage::Worker(WorkerEvent::LogLine(line))) {
        Ok(()) => true,
        Err(TrySendError::Full(_)) => {
            // Count dropped lines so the UI can be told once capacity returns.
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
    let message = format!(
        "Warning: {dropped} log line(s) dropped because the UI was busy",
    );
    // If the UI channel is still full, retain the count for a future flush.
    if let Err(err) = tx.try_send(UiMessage::Worker(WorkerEvent::LogLine(message))) {
        if matches!(err, TrySendError::Full(_)) {
            // Restore the dropped count so the warning is emitted later instead
            // of being lost under sustained saturation.
            DROPPED_LOG_LINES.fetch_add(dropped, Ordering::Relaxed);
        }
    }
}
