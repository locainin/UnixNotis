//! Trial-mode release and previous-owner restoration

use std::time::Duration;

use anyhow::{Context, Result};
use zbus::fdo::DBusProxy;
use zbus::Connection;

use crate::cli::Args;
use crate::dbus_owner::wait_for_owner_state;
use crate::trial_mode::{self, restore_previous, TrialState};
use unixnotis_core::NOTIFICATIONS_BUS_NAME;

pub(super) async fn finish_trial(
    args: &Args,
    connection: &Connection,
    dbus_proxy: &DBusProxy<'_>,
    notifications_name: zbus::names::BusName<'_>,
    trial_state: &mut TrialState,
) -> Result<()> {
    if !args.trial {
        return Ok(());
    }

    // Name release and prior-owner restoration remain independent cleanup duties
    let release_result = connection
        .release_name(NOTIFICATIONS_BUS_NAME)
        .await
        .context("release org.freedesktop.Notifications after trial")
        .map(|_| ());
    let restore_result = restore_trial_owner(
        args,
        dbus_proxy,
        notifications_name,
        trial_state.take_restore_action(),
    )
    .await;
    combine_run_and_restore(release_result, restore_result)
}

async fn restore_trial_owner(
    args: &Args,
    dbus_proxy: &DBusProxy<'_>,
    notifications_name: zbus::names::BusName<'_>,
    action: Option<trial_mode::RestoreAction>,
) -> Result<()> {
    let Some(action) = action else {
        return Ok(());
    };

    restore_previous_or_fail(action)?;
    let reacquired = wait_for_owner_state(
        dbus_proxy,
        notifications_name,
        true,
        Duration::from_millis(args.restore_wait_ms),
    )
    .await
    .context("wait for previous daemon to reacquire org.freedesktop.Notifications")?;
    if !reacquired {
        anyhow::bail!(
            "previous daemon did not reacquire org.freedesktop.Notifications within {} ms",
            args.restore_wait_ms
        );
    }
    Ok(())
}

pub(super) fn combine_run_and_restore(
    run_result: Result<()>,
    restore_result: Result<()>,
) -> Result<()> {
    match (run_result, restore_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(run_error), Ok(())) => Err(run_error),
        (Ok(()), Err(restore_error)) => Err(restore_error),
        (Err(run_error), Err(restore_error)) => {
            Err(run_error.context(format!("trial restoration also failed: {restore_error:#}")))
        }
    }
}

pub(super) fn restore_previous_or_fail(action: trial_mode::RestoreAction) -> Result<()> {
    restore_previous(action).context("restore previous notification daemon")
}
