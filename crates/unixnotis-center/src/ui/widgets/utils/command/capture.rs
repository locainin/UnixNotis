//! Asynchronous request helpers for command output consumers

use std::io;
use std::process::Output;
use std::time::Duration;

use unixnotis_core::{util, PanelDebugLevel};

use super::command_exec::set_command_config_dir;
use super::queue::enqueue_command;
use super::plan::{resolve_command_plan, CommandKind};
use crate::diagnostics::panel_debug as debug;

pub fn configure_command_config_dir(config_dir: std::path::PathBuf) {
    set_command_config_dir(config_dir);
}

pub(in crate::ui::widgets) fn run_command_capture_async(
    cmd: &str,
) -> async_channel::Receiver<Result<Output, io::Error>> {
    enqueue_capture(cmd, CommandKind::Slow, None, "slow")
}

pub(in crate::ui::widgets) fn run_command_capture_with_timeout_async(
    cmd: &str,
    timeout: Duration,
) -> async_channel::Receiver<Result<Output, io::Error>> {
    enqueue_capture(cmd, CommandKind::Slow, Some(timeout), "custom-timeout")
}

pub(in crate::ui::widgets) fn run_command_capture_status_async(
    cmd: &str,
) -> async_channel::Receiver<Result<Output, io::Error>> {
    enqueue_capture(cmd, CommandKind::Fast, None, "fast")
}

fn enqueue_capture(
    cmd: &str,
    kind: CommandKind,
    timeout: Option<Duration>,
    label: &str,
) -> async_channel::Receiver<Result<Output, io::Error>> {
    let (tx, rx) = async_channel::bounded(1);
    let cmd = cmd.trim();
    if cmd.is_empty() {
        let _ = tx.send_blocking(Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "command was empty",
        )));
        return rx;
    }

    let mut plan = resolve_command_plan(cmd, kind);
    if let Some(timeout) = timeout {
        plan = plan.with_timeout(timeout);
    }
    debug::log(PanelDebugLevel::Verbose, || {
        let snippet = util::log_snippet(cmd);
        format!("enqueue {label} command: {snippet}")
    });
    enqueue_command(cmd.to_string(), plan, Some(tx));
    rx
}

#[cfg(test)]
#[path = "tests/capture.rs"]
mod tests;
