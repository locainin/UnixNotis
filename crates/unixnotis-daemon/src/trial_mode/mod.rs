//! Trial-mode helpers for temporarily replacing another notification daemon
//!
//! Keeps detection, stopping, and restoring logic separate from main startup flow

use std::time::Duration;

use anyhow::{anyhow, Result};
use tracing::debug;
use zbus::fdo::DBusProxy;

use super::dbus_owner::wait_for_owner_state;
use super::{Args, RestoreStrategy};

mod control;
mod owner;
mod prompt;

// Re-export keeps main.rs call sites unchanged after the split
pub(super) use control::restore_previous;

#[derive(Default)]
pub(super) struct TrialState {
    // Populated only when the replaced daemon can be restored later
    restore_action: Option<RestoreAction>,
}

impl TrialState {
    pub(super) fn take_restore_action(&mut self) -> Option<RestoreAction> {
        // take moves out the action so restore runs at most once
        self.restore_action.take()
    }
}

pub(super) enum RestoreAction {
    // Restart through the matching user unit
    Systemd { unit: String },
    // Restart with captured command line
    Command { program: String, args: Vec<String> },
}

pub(super) struct OwnerInfo {
    // D-Bus owner PID when available
    pub(super) pid: Option<u32>,
    // Process name from /proc or ps
    pub(super) comm: Option<String>,
    // Full argv for process-based restore
    pub(super) args: Option<Vec<String>>,
}

pub(super) struct DetectedDaemon {
    pub(super) name: String,
    pub(super) systemd_active: bool,
    pub(super) running_pids: Vec<u32>,
    pub(super) is_owner: bool,
}

pub(super) struct KnownDaemon {
    pub(super) name: &'static str,
    pub(super) unit: &'static str,
}

pub(super) const KNOWN_DAEMONS: &[KnownDaemon] = &[
    KnownDaemon {
        name: "mako",
        unit: "mako.service",
    },
    KnownDaemon {
        name: "dunst",
        unit: "dunst.service",
    },
    KnownDaemon {
        name: "swaync",
        unit: "swaync.service",
    },
    KnownDaemon {
        name: "notify-osd",
        unit: "notify-osd.service",
    },
    KnownDaemon {
        name: "quickshell",
        unit: "quickshell.service",
    },
];

pub(super) const TRIAL_COMMAND_TIMEOUT: Duration = Duration::from_secs(2);

pub(super) async fn prepare_trial(
    args: &Args,
    dbus_proxy: &DBusProxy<'_>,
    notifications_name: zbus::names::BusName<'_>,
) -> Result<TrialState> {
    debug!("trial mode detection started");
    // Step 1: resolve the current D-Bus owner for Notifications
    let owner = owner::detect_owner(dbus_proxy, notifications_name.clone()).await?;
    if owner.is_none() {
        debug!("trial mode: no current notification owner");
        return Ok(TrialState::default());
    }

    if let Some(info) = owner.as_ref() {
        debug!(
            pid = info.pid,
            comm = info.comm.as_deref().unwrap_or("unknown"),
            "trial mode: current owner detected"
        );
    }

    // Step 2: collect known daemon status so prompt output is actionable
    let daemons = owner::detect_known_daemons(&owner).await;
    owner::print_detected_daemons(&daemons, &owner);

    if !args.yes {
        // Prompt runs on a blocking worker to keep async runtime responsive
        let confirmed = tokio::task::spawn_blocking(prompt::confirm_trial)
            .await
            .map_err(|err| anyhow!("trial prompt failed: {err}"))??;
        if !confirmed {
            return Err(anyhow!("trial cancelled"));
        }
    }

    let Some(owner) = owner else {
        return Err(anyhow!("no current owner detected for trial mode"));
    };

    // Step 3: stop current owner and capture restore plan when applicable
    let restore_action = control::stop_active_owner(args, &owner).await?;
    // Step 4: wait until bus name is fully released before continuing startup
    let released = wait_for_owner_state(
        dbus_proxy,
        notifications_name,
        false,
        Duration::from_millis(args.restore_wait_ms),
    )
    .await?;
    if !released {
        return Err(anyhow!(
            "org.freedesktop.Notifications did not release in time"
        ));
    }

    debug!("trial mode preparation complete");
    Ok(TrialState { restore_action })
}

#[cfg(test)]
mod tests {
    use super::KNOWN_DAEMONS;

    #[test]
    fn known_daemons_include_quickshell_owner() {
        // Quickshell can own org.freedesktop.Notifications directly
        let quickshell = KNOWN_DAEMONS
            .iter()
            .find(|daemon| daemon.name == "quickshell")
            .expect("quickshell should be known");

        // The unit name lets auto restore prefer systemd when available
        assert_eq!(quickshell.unit, "quickshell.service");
    }
}
