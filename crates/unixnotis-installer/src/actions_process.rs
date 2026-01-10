//! Subprocess execution and log streaming helpers.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::thread;

use anyhow::{Context, Result};

use crate::events::{UiMessage, WorkerEvent};

use super::ActionContext;

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

    let stdout_handle = stdout.map(|stream| {
        let tx = log_tx.clone();
        thread::spawn(move || read_stream(stream, tx))
    });

    let stderr_handle = stderr.map(|stream| {
        let tx = log_tx.clone();
        thread::spawn(move || read_stream(stream, tx))
    });

    let status = child
        .wait()
        .with_context(|| format!("command failed to run: {}", label))?;

    if let Some(handle) = stdout_handle {
        let _ = handle.join();
    }
    if let Some(handle) = stderr_handle {
        let _ = handle.join();
    }

    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("command failed: {}", label))
    }
}

pub fn log_line(ctx: &mut ActionContext, line: impl Into<String>) {
    let _ = ctx
        .log_tx
        .send(UiMessage::Worker(WorkerEvent::LogLine(line.into())));
}

fn sanitize_log_line(line: &str) -> String {
    line.replace('\r', "")
}

fn read_stream(stream: impl std::io::Read, tx: Sender<UiMessage>) {
    let reader = BufReader::new(stream);
    for line in reader.lines().map_while(Result::ok) {
        let _ = tx.send(UiMessage::Worker(WorkerEvent::LogLine(sanitize_log_line(
            &line,
        ))));
    }
}
