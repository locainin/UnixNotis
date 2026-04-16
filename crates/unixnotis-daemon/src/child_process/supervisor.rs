//! Supervisor loop for the popups and center child processes

use std::process::ExitStatus;
use std::time::{Duration, Instant};

use tokio::process::Child;
use tokio::sync::watch;
use tokio::time::sleep;
use tracing::{info, warn};

#[cfg(unix)]
use rustix::process::{kill_process, Pid, Signal};

use super::{RestartBackoff, UiProcessKind};
use crate::daemon::DaemonState;
use crate::Args;

pub(super) async fn supervise_process(
    kind: UiProcessKind,
    args: Args,
    state: std::sync::Arc<DaemonState>,
    mut shutdown: watch::Receiver<bool>,
) {
    let label = kind.label();
    let mut backoff = RestartBackoff::new();

    loop {
        if shutdown_is_terminal(None, &mut shutdown) {
            kind.mark_running(&state, false);
            return;
        }

        let mut child = match kind.start(&args) {
            Ok(child) => child,
            Err(err) => {
                kind.mark_running(&state, false);
                let delay = backoff.next_delay(Duration::ZERO);
                warn!(
                    ?err,
                    delay_ms = delay.as_millis() as u64,
                    process = label,
                    "ui process failed to start"
                );
                if wait_for_retry_or_shutdown(delay, &mut shutdown).await {
                    return;
                }
                continue;
            }
        };

        let pid = child.id().unwrap_or_default();
        let started_at = Instant::now();
        kind.mark_running(&state, true);
        info!(pid, process = label, "ui process started");

        tokio::select! {
            status = child.wait() => {
                kind.mark_running(&state, false);
                let runtime = started_at.elapsed();
                if !handle_wait_result(&mut child, label, pid, runtime, status).await {
                    return;
                }
                let delay = backoff.next_delay(runtime);
                warn!(
                    delay_ms = delay.as_millis() as u64,
                    process = label,
                    "ui process will be restarted"
                );
                if wait_for_retry_or_shutdown(delay, &mut shutdown).await {
                    return;
                }
            }
            changed = shutdown.changed() => {
                kind.mark_running(&state, false);
                if shutdown_is_terminal(Some(changed), &mut shutdown) {
                    terminate_child(&mut child, label).await;
                    return;
                }
            }
        }
    }
}

fn log_exit(label: &str, pid: u32, runtime: Duration, status: std::io::Result<ExitStatus>) {
    match status {
        Ok(status) => {
            warn!(
                pid,
                process = label,
                runtime_ms = runtime.as_millis() as u64,
                status = %status,
                "ui process exited"
            );
        }
        Err(err) => {
            warn!(
                ?err,
                pid,
                process = label,
                runtime_ms = runtime.as_millis() as u64,
                "ui process wait failed"
            );
        }
    }
}

async fn handle_wait_result(
    child: &mut Child,
    label: &str,
    pid: u32,
    runtime: Duration,
    status: std::io::Result<ExitStatus>,
) -> bool {
    match status {
        Ok(status) => {
            log_exit(label, pid, runtime, Ok(status));
            true
        }
        Err(err) => {
            let probe = child.try_wait().map(|status| status.is_some());
            // Restart only after the child is known dead
            // An unknown wait state can leave two UI processes alive at once
            if wait_error_needs_recovery(&probe) {
                warn!(
                    ?err,
                    pid,
                    process = label,
                    runtime_ms = runtime.as_millis() as u64,
                    "ui process wait failed before exit was confirmed; terminating child before restart"
                );
                terminate_child(child, label).await;
                return true;
            }
            warn!(
                ?err,
                pid,
                process = label,
                runtime_ms = runtime.as_millis() as u64,
                "ui process wait failed but exit was confirmed"
            );
            true
        }
    }
}

fn wait_error_needs_recovery(probe: &std::io::Result<bool>) -> bool {
    matches!(probe, Ok(false) | Err(_))
}

fn shutdown_is_terminal(
    changed: Option<Result<(), watch::error::RecvError>>,
    shutdown: &mut watch::Receiver<bool>,
) -> bool {
    // A closed watch channel means the supervisor owner is gone
    // That should stop restarts the same way as an explicit true flag
    if changed.is_some_and(|result| result.is_err()) {
        return true;
    }
    if *shutdown.borrow() {
        return true;
    }
    shutdown.has_changed().is_err()
}

async fn wait_for_retry_or_shutdown(delay: Duration, shutdown: &mut watch::Receiver<bool>) -> bool {
    // Zero-delay restarts recover fast after a long healthy run
    if delay.is_zero() {
        return shutdown_is_terminal(None, shutdown);
    }

    tokio::select! {
        _ = sleep(delay) => false,
        changed = shutdown.changed() => {
            shutdown_is_terminal(Some(changed), shutdown)
        }
    }
}

async fn terminate_child(child: &mut Child, label: &str) {
    if let Ok(Some(_)) = child.try_wait() {
        return;
    }

    let pid = child.id().unwrap_or_default();
    #[cfg(unix)]
    {
        let pid = match i32::try_from(pid) {
            Ok(pid) => pid,
            Err(_) => {
                warn!(label, pid, "pid exceeds i32 range; skipping SIGTERM");
                return;
            }
        };
        if let Some(pid) = Pid::from_raw(pid) {
            let _ = kill_process(pid, Signal::TERM);
        }
    }

    let start = Instant::now();
    let timeout = Duration::from_millis(600);
    while start.elapsed() < timeout {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {}
            Err(err) => {
                warn!(
                    ?err,
                    label, pid, "failed to poll child state during shutdown"
                );
                break;
            }
        }
        // Small async waits keep shutdown responsive
        sleep(Duration::from_millis(50)).await;
    }

    warn!(label, pid, "force killing unresponsive child process");
    if let Err(err) = child.kill().await {
        warn!(?err, label, pid, "failed to kill child process");
    }
    let _ = child.wait().await;
}

#[cfg(test)]
mod tests {
    use tokio::sync::watch;

    use super::{shutdown_is_terminal, wait_error_needs_recovery};
    use crate::child_process::{
        RestartBackoff, HEALTHY_RUNTIME_SECS, RESTART_BASE_MS, RESTART_MAX_MS,
    };
    use std::time::Duration;

    #[test]
    fn crash_loop_backoff_starts_at_base() {
        let mut backoff = RestartBackoff::new();
        assert_eq!(
            backoff.next_delay(Duration::from_secs(0)),
            Duration::from_millis(RESTART_BASE_MS)
        );
    }

    #[test]
    fn crash_loop_backoff_caps_at_max() {
        let mut backoff = RestartBackoff::new();
        for _ in 0..8 {
            let _ = backoff.next_delay(Duration::from_secs(0));
        }
        assert_eq!(backoff.current, Duration::from_millis(RESTART_MAX_MS));
    }

    #[test]
    fn healthy_runtime_restarts_immediately() {
        let mut backoff = RestartBackoff::new();
        let _ = backoff.next_delay(Duration::from_secs(0));
        assert_eq!(
            backoff.next_delay(Duration::from_secs(HEALTHY_RUNTIME_SECS)),
            Duration::ZERO
        );
    }

    #[test]
    fn wait_error_needs_recovery_when_child_state_is_unknown() {
        assert!(wait_error_needs_recovery(&Ok(false)));
        assert!(wait_error_needs_recovery(&Err(std::io::Error::other(
            "probe failed"
        ))));
        assert!(!wait_error_needs_recovery(&Ok(true)));
    }

    #[test]
    fn shutdown_is_terminal_when_channel_is_closed() {
        let (tx, rx) = watch::channel(false);
        drop(tx);
        let mut rx = rx;
        assert!(shutdown_is_terminal(None, &mut rx));
    }

    #[test]
    fn shutdown_is_terminal_when_flag_is_true() {
        let (_tx, rx) = watch::channel(true);
        let mut rx = rx;
        assert!(shutdown_is_terminal(None, &mut rx));
    }
}
