//! Bounded checks that prove the notification runtime is no longer active

use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};

pub const STOP_QUIESCENCE_TIMEOUT: Duration = Duration::from_secs(5);
const STOP_QUIESCENCE_POLL_INTERVAL: Duration = Duration::from_millis(25);

fn ensure_no_conflicting_live_daemon_until(
    paths: &crate::paths::InstallPaths,
    deadline: Instant,
) -> Result<()> {
    // This check runs at the final generation-switch boundary, not from the UI snapshot
    let owner = crate::detect::notification_owner_for_mutation_until(deadline)
        .context("recheck notification ownership before binary activation")?;
    if let Some(owner) = owner {
        return Err(anyhow!(
            "notification daemon appeared before binary activation (owner {owner}); retry installation"
        ));
    }
    ensure_selected_service_inactive_until(paths, deadline)
}

pub(in crate::actions) fn ensure_selected_service_inactive_until(
    paths: &crate::paths::InstallPaths,
    deadline: Instant,
) -> Result<()> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "selected service probe deadline elapsed",
        )
        .into());
    }
    let state = paths
        .service
        .active_probe()
        .evaluate_state_with_timeout(remaining)
        .context("recheck selected service manager before binary activation")?;
    match state {
        crate::service_manager::contract::ServiceProbeState::Absent
        | crate::service_manager::contract::ServiceProbeState::Inactive => Ok(()),
        crate::service_manager::contract::ServiceProbeState::Active => Err(anyhow!(
            "UnixNotis service became active again before binary activation"
        )),
        crate::service_manager::contract::ServiceProbeState::Unavailable => Err(anyhow!(
            "selected service manager became unavailable before binary activation"
        )),
        crate::service_manager::contract::ServiceProbeState::Indeterminate => Err(anyhow!(
            "selected service manager returned an indeterminate state before binary activation"
        )),
    }
}

pub fn ensure_selected_service_inactive(paths: &crate::paths::InstallPaths) -> Result<()> {
    let deadline = Instant::now()
        .checked_add(crate::service_manager::contract::ServiceProbe::default_timeout())
        .ok_or_else(|| anyhow!("selected service check deadline exceeded the monotonic clock"))?;
    ensure_selected_service_inactive_until(paths, deadline)
}

pub fn wait_until_no_conflicting_live_daemon(
    paths: &crate::paths::InstallPaths,
    timeout: Duration,
) -> Result<()> {
    wait_until_no_conflicting_live_daemon_with_probe(
        timeout,
        STOP_QUIESCENCE_POLL_INTERVAL,
        |deadline| ensure_no_conflicting_live_daemon_until(paths, deadline),
    )
}

pub fn wait_until_selected_service_inactive(
    paths: &crate::paths::InstallPaths,
    timeout: Duration,
) -> Result<()> {
    // A held activation reservation makes broker ownership intentionally non-empty
    wait_until_no_conflicting_live_daemon_with_probe(
        timeout,
        STOP_QUIESCENCE_POLL_INTERVAL,
        |deadline| ensure_selected_service_inactive_until(paths, deadline),
    )
}

fn wait_until_no_conflicting_live_daemon_with_probe<F>(
    timeout: Duration,
    poll_interval: Duration,
    mut probe: F,
) -> Result<()>
where
    F: FnMut(Instant) -> Result<()>,
{
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| anyhow!("daemon quiescence deadline exceeded the monotonic clock"))?;
    let poll_interval = poll_interval.max(Duration::from_millis(1));
    let max_attempts = timeout
        .as_nanos()
        .checked_div(poll_interval.as_nanos())
        .unwrap_or(0)
        .saturating_add(1);
    let max_attempts = usize::try_from(max_attempts).unwrap_or(usize::MAX);
    let mut last_error = None;
    for _attempt in 0..max_attempts {
        match probe(deadline) {
            Ok(()) => return Ok(()),
            Err(error) => {
                let now = Instant::now();
                last_error = Some(error);
                let remaining = deadline.saturating_duration_since(now);
                if remaining.is_zero() {
                    break;
                }
                // Bounded polling handles service-manager success before broker ownership disappears
                std::thread::sleep(poll_interval.min(remaining));
            }
        }
    }
    let error = last_error.ok_or_else(|| anyhow!("daemon quiescence probe did not run"))?;
    Err(error).context("notification runtime did not become quiescent before rollback deadline")
}

#[cfg(test)]
#[path = "tests/quiescence.rs"]
mod tests;
