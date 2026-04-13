//! Command execution, budgeting, and watch helpers for widgets.

mod action;
mod command_exec;
mod command_parse;
mod command_queue;

use std::io;
use std::process::{Child, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use unixnotis_core::util;
use unixnotis_core::PanelDebugLevel;

use crate::debug;

pub(in crate::ui::widgets) use action::run_action_command_with_completion;
use command_exec::build_command;
use command_parse::is_probably_slow;
use command_queue::enqueue_command;

pub(in crate::ui::widgets) use command_exec::kill_process_group;

// Timeout budgets are tuned to keep UI responsive while allowing slow shell
// commands enough time to finish without spamming retries.
const FAST_TIMEOUT_MS: u64 = 350;
const SLOW_TIMEOUT_MS: u64 = 800;
const ACTION_TIMEOUT_MS: u64 = 1200;
// Slow command jitter avoids synchronized polling across widgets.
const SLOW_JITTER_MS: u64 = 200;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(in crate::ui::widgets) enum CommandKind {
    // Fast probes such as state checks
    Fast,
    // Potentially expensive reads that may involve D-Bus or shell pipelines
    Slow,
    // User-triggered actions where responsiveness matters more than throughput
    Action,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::ui::widgets) struct CommandPlan {
    kind: CommandKind,
    timeout_override: Option<Duration>,
}

impl CommandPlan {
    fn timeout(self) -> Duration {
        // Explicit timeout override is used by plugin-backed widgets.
        if let Some(timeout) = self.timeout_override {
            return timeout;
        }
        match self.kind {
            CommandKind::Fast => Duration::from_millis(FAST_TIMEOUT_MS),
            CommandKind::Slow => Duration::from_millis(SLOW_TIMEOUT_MS),
            CommandKind::Action => Duration::from_millis(ACTION_TIMEOUT_MS),
        }
    }

    fn jitter(self) -> Duration {
        // Jitter applies only to slow commands to desynchronize refresh bursts
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
        // Watch commands must keep stdout open for streaming; stderr is suppressed
        // to avoid spurious wakeups for noisy utilities.
        let mut command = build_command(cmd);
        command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        command.spawn()
    }

    fn with_timeout(self, timeout: Duration) -> Self {
        Self {
            timeout_override: Some(timeout),
            ..self
        }
    }
}

pub(in crate::ui::widgets) fn resolve_command_plan(
    cmd: &str,
    default_kind: CommandKind,
) -> CommandPlan {
    // Start with caller intent and upgrade only when heuristics require it
    let mut kind = default_kind;
    // Action commands remain action-class even if the heuristic marks them slow.
    if default_kind != CommandKind::Action && is_probably_slow(cmd) {
        kind = CommandKind::Slow;
    }
    CommandPlan {
        kind,
        timeout_override: None,
    }
}

pub(in crate::ui::widgets) fn run_command_capture_async(
    cmd: &str,
) -> async_channel::Receiver<Result<Output, io::Error>> {
    // Single-result channel keeps queue semantics simple for UI callers
    let (tx, rx) = async_channel::bounded(1);
    let cmd = cmd.trim();
    if cmd.is_empty() {
        // Preserve error semantics on the receiver even when the command is invalid.
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

pub(in crate::ui::widgets) fn run_command_capture_with_timeout_async(
    cmd: &str,
    timeout: Duration,
) -> async_channel::Receiver<Result<Output, io::Error>> {
    // Timeout-aware variant is used primarily by plugin-backed widgets
    let (tx, rx) = async_channel::bounded(1);
    let cmd = cmd.trim();
    if cmd.is_empty() {
        // Preserve receiver error semantics on invalid input.
        let _ = tx.send_blocking(Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "command was empty",
        )));
        return rx;
    }
    // Reuse queueing and slow-command heuristics with only timeout overridden.
    let plan = resolve_command_plan(cmd, CommandKind::Slow).with_timeout(timeout);
    debug::log(PanelDebugLevel::Verbose, || {
        let snippet = util::log_snippet(cmd);
        format!("enqueue custom-timeout command: {snippet}")
    });
    enqueue_command(cmd.to_string(), plan, Some(tx));
    rx
}

pub(in crate::ui::widgets) fn run_command_capture_status_async(
    cmd: &str,
) -> async_channel::Receiver<Result<Output, io::Error>> {
    // Status probe path uses a smaller default timeout budget
    let (tx, rx) = async_channel::bounded(1);
    let cmd = cmd.trim();
    if cmd.is_empty() {
        // Keep the receiver behavior consistent with the non-empty path.
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
