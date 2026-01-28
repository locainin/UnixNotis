//! Command execution, budgeting, and watch helpers for widgets.

mod command_exec;
mod command_parse;
mod command_queue;

use std::io;
use std::process::{Child, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tracing::warn;
use unixnotis_core::util;
use unixnotis_core::PanelDebugLevel;

use crate::debug;

use command_exec::build_command;
use command_parse::is_probably_slow;
use command_queue::enqueue_command;

pub(in crate::ui::widgets) use command_exec::kill_process_group;

const FAST_TIMEOUT_MS: u64 = 350;
const SLOW_TIMEOUT_MS: u64 = 800;
const ACTION_TIMEOUT_MS: u64 = 1200;
const SLOW_JITTER_MS: u64 = 200;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::ui::widgets) enum CommandKind {
    Fast,
    Slow,
    Action,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::ui::widgets) struct CommandPlan {
    kind: CommandKind,
}

impl CommandPlan {
    fn timeout(self) -> Duration {
        match self.kind {
            CommandKind::Fast => Duration::from_millis(FAST_TIMEOUT_MS),
            CommandKind::Slow => Duration::from_millis(SLOW_TIMEOUT_MS),
            CommandKind::Action => Duration::from_millis(ACTION_TIMEOUT_MS),
        }
    }

    fn jitter(self) -> Duration {
        if self.kind != CommandKind::Slow || SLOW_JITTER_MS == 0 {
            return Duration::from_millis(0);
        }
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as u64;
        let jitter_ms = (nanos % (SLOW_JITTER_MS * 1_000_000)) / 1_000_000;
        Duration::from_millis(jitter_ms)
    }

    pub(in crate::ui::widgets) fn spawn_watch_command(&self, cmd: &str) -> io::Result<Child> {
        let mut command = build_command(cmd);
        command.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
        command.spawn()
    }
}

pub(in crate::ui::widgets) fn resolve_command_plan(
    cmd: &str,
    default_kind: CommandKind,
) -> CommandPlan {
    let mut kind = default_kind;
    if default_kind != CommandKind::Action && is_probably_slow(cmd) {
        kind = CommandKind::Slow;
    }
    CommandPlan { kind }
}

pub(in crate::ui::widgets) fn run_command(cmd: &str) {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        warn!("command was empty");
        return;
    }
    debug::log(PanelDebugLevel::Verbose, || {
        let snippet = util::log_snippet(cmd);
        format!("enqueue action command: {snippet}")
    });
    enqueue_command(
        cmd.to_string(),
        resolve_command_plan(cmd, CommandKind::Action),
        None,
    );
}

pub(in crate::ui::widgets) fn run_command_capture_async(
    cmd: &str,
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
    let plan = resolve_command_plan(cmd, CommandKind::Slow);
    debug::log(PanelDebugLevel::Verbose, || {
        let snippet = util::log_snippet(cmd);
        format!("enqueue slow command: {snippet}")
    });
    enqueue_command(cmd.to_string(), plan, Some(tx));
    rx
}

pub(in crate::ui::widgets) fn run_command_capture_status_async(
    cmd: &str,
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
    let plan = resolve_command_plan(cmd, CommandKind::Fast);
    debug::log(PanelDebugLevel::Verbose, || {
        let snippet = util::log_snippet(cmd);
        format!("enqueue fast command: {snippet}")
    });
    enqueue_command(cmd.to_string(), plan, Some(tx));
    rx
}
