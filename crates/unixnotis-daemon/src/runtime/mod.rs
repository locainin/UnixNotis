//! Daemon runtime and trial cleanup coordination

mod daemon;
mod shutdown;
mod trial_cleanup;

use anyhow::{Context, Result};
use zbus::fdo::DBusProxy;
use zbus::Connection;

use crate::cli::Args;
use crate::trial_mode::{prepare_trial, TrialState};
use unixnotis_core::Config;

pub async fn run(args: &Args, config: Config) -> Result<()> {
    let connection = Connection::session()
        .await
        .context("connect to session bus")?;
    let dbus_proxy = DBusProxy::new(&connection).await?;
    let notifications_name = zbus::names::BusName::try_from("org.freedesktop.Notifications")?;
    let mut trial_state = if args.trial {
        prepare_trial(args, &dbus_proxy, notifications_name.clone()).await?
    } else {
        TrialState::default()
    };

    // Trial cleanup runs after every daemon result, including partial startup failures
    let run_result = daemon::run_daemon(
        args,
        config,
        &connection,
        &dbus_proxy,
        notifications_name.clone(),
    )
    .await;
    let restore_result = trial_cleanup::finish_trial(
        args,
        &connection,
        &dbus_proxy,
        notifications_name,
        &mut trial_state,
    )
    .await;
    trial_cleanup::combine_run_and_restore(run_result, restore_result)
}

#[cfg(test)]
mod tests;
