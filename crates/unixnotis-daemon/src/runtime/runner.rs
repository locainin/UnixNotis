//! Daemon runtime and trial cleanup coordination

use std::sync::Arc;

use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use zbus::connection::Builder;
use zbus::fdo::DBusProxy;

use crate::cli::Args;
use crate::daemon::{DesktopIdentityIndex, DesktopIndexSnapshot};
use crate::trial_mode::{prepare_trial, TrialState};
use unixnotis_core::{log_session_bus_identity, Config, NOTIFICATIONS_BUS_NAME};

use super::{daemon, trial_cleanup};

const DAEMON_DBUS_QUEUE_CAPACITY: usize = 16;

pub async fn run(args: &Args, config: Config) -> Result<()> {
    let builder = Builder::session().context("create session bus connection")?;
    Box::pin(run_with_builder(args, config, builder)).await
}

async fn run_with_builder(args: &Args, config: Config, builder: Builder<'_>) -> Result<()> {
    Box::pin(run_with_builder_inner(args, config, builder, None)).await
}

#[cfg(test)]
async fn run_with_builder_for_test(
    args: &Args,
    config: Config,
    builder: Builder<'_>,
    trusted_control_sender: String,
) -> Result<()> {
    Box::pin(run_with_builder_inner(
        args,
        config,
        builder,
        Some(trusted_control_sender),
    ))
    .await
}

async fn run_with_builder_inner(
    args: &Args,
    config: Config,
    builder: Builder<'_>,
    trusted_test_control_sender: Option<String>,
) -> Result<()> {
    let connection = builder
        .max_queued(DAEMON_DBUS_QUEUE_CAPACITY)
        .build()
        .await
        .context("connect to session bus")?;
    log_session_bus_identity(&connection, "daemon")
        .await
        .context("read daemon session-bus identity")?;
    // Finish the bounded filesystem scan before either well-known name can become visible
    let desktop_index_snapshot = tokio::task::spawn_blocking(DesktopIdentityIndex::build_snapshot)
        .await
        .context("desktop identity index task failed")?;
    let DesktopIndexSnapshot {
        index: desktop_identity_index,
        watched_directories,
    } = desktop_index_snapshot;
    let desktop_identity_index = Arc::new(ArcSwap::from_pointee(desktop_identity_index));
    let dbus_proxy = DBusProxy::new(&connection).await?;
    let notifications_name = zbus::names::BusName::try_from(NOTIFICATIONS_BUS_NAME)?;
    let mut trial_state = if trial_requested(args) {
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
        desktop_identity_index,
        watched_directories,
        trusted_test_control_sender,
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

const fn trial_requested(args: &Args) -> bool {
    args.trial
}

#[cfg(test)]
#[path = "tests/runner.rs"]
mod tests;
