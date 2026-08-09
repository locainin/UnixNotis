//! Trial-mode helpers for temporarily replacing another notification daemon
//!
//! Keeps detection, stopping, and restoring logic separate from main startup flow

use std::time::Duration;

use anyhow::{anyhow, Result};
use tracing::debug;
use zbus::fdo::DBusProxy;

use crate::cli::Args;
use crate::daemon::wait_for_owner_state;

use super::{control, owner, prompt};

#[derive(Default)]
pub struct TrialState {
    // Populated only when the replaced daemon can be restored later
    restore_action: Option<RestoreAction>,
}

impl TrialState {
    pub(crate) const fn take_restore_action(&mut self) -> Option<RestoreAction> {
        // take moves out the action so restore runs at most once
        self.restore_action.take()
    }
}

#[derive(Debug)]
pub enum RestoreAction {
    // Restart through the matching user unit
    Systemd { unit: String },
    // Restart with captured command line
    Command { program: String, args: Vec<String> },
}

pub struct OwnerInfo {
    // Exact broker address is revalidated immediately before any stop operation
    pub(super) unique_name: String,
    // D-Bus owner PID when available
    pub(super) pid: Option<u32>,
    // Process name from /proc or ps
    pub(super) comm: Option<String>,
    // Full argv for process-based restore
    pub(super) args: Option<Vec<String>>,
}

pub(in crate::trial_mode) enum NotificationOwnerState {
    Unowned,
    Owned(OwnerInfo),
}

pub struct DetectedDaemon {
    pub(super) name: String,
    pub(super) systemd_active: bool,
    pub(super) running_pids: Vec<u32>,
    pub(super) is_owner: bool,
}

pub const KNOWN_DAEMONS: &[unixnotis_core::KnownNotificationDaemon] =
    unixnotis_core::KNOWN_NOTIFICATION_DAEMONS;

pub const TRIAL_COMMAND_TIMEOUT: Duration = Duration::from_secs(2);

pub async fn prepare_trial(
    args: &Args,
    dbus_proxy: &DBusProxy<'_>,
    notifications_name: zbus::names::BusName<'_>,
) -> Result<TrialState> {
    debug!("trial mode detection started");
    // Step 1: resolve the current D-Bus owner for Notifications
    let owner = match owner::detect_owner(dbus_proxy, notifications_name.clone()).await? {
        NotificationOwnerState::Unowned => {
            debug!("trial mode: no current notification owner");
            return Ok(TrialState::default());
        }
        NotificationOwnerState::Owned(owner) => owner,
    };

    debug!(
        pid = owner.pid,
        comm = owner.comm.as_deref().unwrap_or("unknown"),
        "trial mode: current owner detected"
    );

    // Step 2: collect known daemon status so prompt output is actionable
    let owner_view = Some(owner);
    let daemons = owner::detect_known_daemons(&owner_view).await;
    owner::print_detected_daemons(&daemons, &owner_view);

    if !args.yes {
        // Prompt runs on a blocking worker to keep async runtime responsive
        let confirmed = tokio::task::spawn_blocking(prompt::confirm_trial)
            .await
            .map_err(|err| anyhow!("trial prompt failed: {err}"))??;
        if !confirmed {
            return Err(anyhow!("trial cancelled"));
        }
    }

    let Some(owner) = owner_view else {
        return Err(anyhow!("trial owner state disappeared before revalidation"));
    };
    owner::ensure_owner_is_current(dbus_proxy, notifications_name.clone(), &owner).await?;

    // Step 3: stop current owner and capture restore plan when applicable
    let restore_action = control::stop_active_owner(args, &owner).await?;
    let mut trial_state = TrialState { restore_action };
    // Step 4: wait until bus name is fully released before continuing startup
    let released = wait_for_owner_state(
        dbus_proxy,
        notifications_name,
        false,
        Duration::from_millis(args.restore_wait_ms),
    )
    .await;
    let released = match released {
        Ok(released) => released,
        Err(wait_error) => {
            return Err(restore_after_prepare_failure(&mut trial_state, wait_error));
        }
    };
    if !released {
        return Err(restore_after_prepare_failure(
            &mut trial_state,
            anyhow!("org.freedesktop.Notifications did not release in time"),
        ));
    }

    debug!("trial mode preparation complete");
    Ok(trial_state)
}

fn restore_after_prepare_failure(
    trial_state: &mut TrialState,
    prepare_error: anyhow::Error,
) -> anyhow::Error {
    let Some(action) = trial_state.take_restore_action() else {
        return prepare_error;
    };

    // Preparation owns cleanup until a complete TrialState can be returned to main
    match control::restore_previous(action) {
        Ok(()) => prepare_error,
        Err(restore_error) => {
            prepare_error.context(format!("trial restoration also failed: {restore_error:#}"))
        }
    }
}

#[cfg(test)]
#[path = "tests/known_daemons.rs"]
mod known_daemons_tests;
#[cfg(test)]
#[path = "tests/state.rs"]
mod tests;
