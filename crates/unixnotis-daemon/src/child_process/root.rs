//! Child process management for UI components
//!
//! Keeps spawn, restart, and shutdown logic for popups and center processes in one place

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{anyhow, Result};
use tokio::process::{Child, Command};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use unixnotis_core::util::CONFIG_PATH_ENV;

use super::Args;
use crate::daemon::DaemonState;

#[path = "paths.rs"]
mod paths;
#[path = "supervisor.rs"]
mod supervisor;

use paths::{apply_parent_death_signal, resolve_center_path, resolve_popups_path};
use supervisor::supervise_process;

// A short loop should not hammer respawns forever
// A long healthy run should restart right away
const RESTART_BASE_MS: u64 = 250;
const RESTART_MAX_MS: u64 = 5000;
const HEALTHY_RUNTIME_SECS: u64 = 30;

#[derive(Clone, Copy, Debug)]
enum UiProcessKind {
    Popups,
    Center,
}

impl UiProcessKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Popups => "unixnotis-popups",
            Self::Center => "unixnotis-center",
        }
    }

    fn mark_running(self, state: &DaemonState, running: bool) {
        match self {
            Self::Popups => state.set_popups_running(running),
            Self::Center => {
                let _ = running;
                // Center readiness is tied to live subscriptions
                // A spawned process alone is not enough to mark it ready
                // Spawned is not the same as subscribed and ready
                // The center flips this to true once its control streams are active
                state.set_panel_ready(false);
            }
        }
    }

    fn build_command(self, args: &Args) -> Command {
        let mut command = match self {
            Self::Popups => {
                if let Some(path) = resolve_popups_path() {
                    Command::new(path)
                } else {
                    Command::new("unixnotis-popups")
                }
            }
            Self::Center => {
                if let Some(path) = resolve_center_path() {
                    Command::new(path)
                } else {
                    Command::new("unixnotis-center")
                }
            }
        };

        // Journal should keep child logs tied to the daemon service
        // Inherited output makes crash lines easier to trace later
        command.stdin(Stdio::null());
        command.stdout(Stdio::inherit());
        command.stderr(Stdio::inherit());

        apply_parent_death_signal(&mut command);

        if let Some(config) = args.config.as_ref() {
            // GTK re-parses argv in child apps, so custom config paths travel by env instead
            command.env(CONFIG_PATH_ENV, child_config_env_path(config));
        } else {
            // Clear inherited overrides so default child launches stay predictable
            command.env_remove(CONFIG_PATH_ENV);
        }

        command
    }

    fn start(self, args: &Args) -> Result<Child> {
        let mut command = self.build_command(args);
        let label = self.label();
        command.spawn().map_err(|err| {
            anyhow!("failed to start {label} ({err}); build it or install it on PATH")
        })
    }
}

fn child_config_env_path(config: &Path) -> PathBuf {
    if config.is_absolute() {
        return config.to_path_buf();
    }

    // Child processes may start from a different working dir later, so make the path stable now
    std::env::current_dir().map_or_else(|_| config.to_path_buf(), |cwd| cwd.join(config))
}

#[derive(Debug)]
struct RestartBackoff {
    current: Duration,
}

impl RestartBackoff {
    const fn new() -> Self {
        Self {
            current: Duration::ZERO,
        }
    }

    fn next_delay(&mut self, runtime: Duration) -> Duration {
        // A healthy long run should come back fast
        if runtime >= Duration::from_secs(HEALTHY_RUNTIME_SECS) {
            self.current = Duration::ZERO;
            return Duration::ZERO;
        }

        let base = Duration::from_millis(RESTART_BASE_MS);
        let max = Duration::from_millis(RESTART_MAX_MS);
        let delay = if self.current.is_zero() {
            base
        } else {
            self.current
        };

        self.current = delay.checked_mul(2).unwrap_or(max).min(max);
        delay
    }
}

pub fn spawn_popups_supervisor(
    args: Args,
    state: std::sync::Arc<DaemonState>,
    shutdown: watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        supervise_process(UiProcessKind::Popups, args, state, shutdown).await;
    })
}

pub fn spawn_center_supervisor(
    args: Args,
    state: std::sync::Arc<DaemonState>,
    shutdown: watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        supervise_process(UiProcessKind::Center, args, state, shutdown).await;
    })
}

#[cfg(test)]
#[path = "tests/command.rs"]
mod tests;
