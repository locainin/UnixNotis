//! Command classification and execution budgets

use std::io;
use std::process::Child;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use unixnotis_core::CommandSpec;

use super::command_parse::is_probably_slow;
use super::exec::build_command;

const FAST_TIMEOUT_MS: u64 = 350;
const SLOW_TIMEOUT_MS: u64 = 800;
const ACTION_TIMEOUT_MS: u64 = 1_200;
const SLOW_JITTER_MS: u64 = 200;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(in crate::ui::widgets) enum CommandKind {
    // Fast probes such as state checks
    Fast,
    // Potentially expensive reads that may involve D-Bus or shell pipelines
    Slow,
    // User-triggered actions use a worker lane independent of refresh throughput
    Action,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::ui::widgets) struct CommandPlan {
    pub(super) kind: CommandKind,
    pub(super) timeout_override: Option<Duration>,
}

impl CommandPlan {
    pub(super) const fn timeout(self) -> Duration {
        if let Some(timeout) = self.timeout_override {
            return timeout;
        }
        match self.kind {
            CommandKind::Fast => Duration::from_millis(FAST_TIMEOUT_MS),
            CommandKind::Slow => Duration::from_millis(SLOW_TIMEOUT_MS),
            CommandKind::Action => Duration::from_millis(ACTION_TIMEOUT_MS),
        }
    }

    pub(super) fn jitter(self) -> Duration {
        if self.kind != CommandKind::Slow || SLOW_JITTER_MS == 0 {
            return Duration::ZERO;
        }
        let nanos = u64::from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos(),
        );
        let jitter_ms = (nanos % (SLOW_JITTER_MS * 1_000_000)) / 1_000_000;
        Duration::from_millis(jitter_ms)
    }

    pub(in crate::ui::widgets) fn spawn_watch_command(
        &self,
        cmd: &CommandSpec,
    ) -> io::Result<Child> {
        // Watch commands keep stdout open while stderr stays detached from refresh wakeups
        let mut command = build_command(cmd);
        command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        command.spawn()
    }

    pub(super) const fn with_timeout(self, timeout: Duration) -> Self {
        Self {
            timeout_override: Some(timeout),
            ..self
        }
    }
}

pub(in crate::ui::widgets) fn resolve_command_plan(
    cmd: &CommandSpec,
    default_kind: CommandKind,
) -> CommandPlan {
    let mut kind = default_kind;
    // User actions keep their dedicated lane even when their command looks expensive
    if default_kind != CommandKind::Action && is_probably_slow(cmd) {
        kind = CommandKind::Slow;
    }
    CommandPlan {
        kind,
        timeout_override: None,
    }
}

#[cfg(test)]
#[path = "tests/plan.rs"]
mod tests;
