//! D-Bus owner tracking helpers
//!
//! Provides reusable helpers for name ownership checks during startup and trial mode

use std::time::Duration;

use anyhow::Result;
use futures_util::StreamExt;
use tracing::{info, warn};
use zbus::fdo::DBusProxy;
use zbus::Connection;

pub(super) async fn wait_for_owner_state(
    dbus_proxy: &DBusProxy<'_>,
    name: zbus::names::BusName<'_>,
    expect_owner: bool,
    timeout: Duration,
) -> Result<bool> {
    let name_str = name.to_string();
    let mut stream = dbus_proxy
        .receive_name_owner_changed_with_args(&[(0, name_str.as_str())])
        .await?;
    // Re-check after subscribing to avoid missing a transition between the initial query and stream setup
    let has_owner = match dbus_proxy.name_has_owner(name.clone()).await {
        Ok(value) => value,
        Err(err) => {
            warn!(?err, "failed to query D-Bus owner state");
            false
        }
    };
    if has_owner == expect_owner {
        return Ok(true);
    }
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => return Ok(false),
            signal = stream.next() => {
                let Some(signal) = signal else {
                    return Ok(false);
                };
                let args = signal.args()?;
                let new_owner = args
                    .new_owner()
                    .as_ref()
                    .map(|name| name.as_str())
                    .unwrap_or("");
                if owner_state_matches(Some(new_owner), expect_owner) {
                    return Ok(true);
                }
            }
        }
    }
}

pub(super) async fn log_current_owner(
    dbus_proxy: &DBusProxy<'_>,
    connection: &Connection,
    name: zbus::names::BusName<'_>,
) -> Result<bool> {
    let unique_name = connection.unique_name().map(|name| name.to_string());
    let owner = match dbus_proxy.get_name_owner(name).await {
        Ok(owner) => owner.to_string(),
        Err(err) => {
            info!(?err, "org.freedesktop.Notifications has no owner");
            return Ok(false);
        }
    };
    let is_self = owner_name_is_self(unique_name.as_deref(), owner.as_str());
    if is_self {
        info!(owner, "org.freedesktop.Notifications owner (self)");
    } else {
        info!(owner, "org.freedesktop.Notifications owner");
    }
    Ok(is_self)
}

fn owner_state_matches(new_owner: Option<&str>, expect_owner: bool) -> bool {
    // D-Bus signals encode release as an empty owner name, not as a missing signal
    let has_owner = new_owner.is_some_and(|name| !name.is_empty());
    has_owner == expect_owner
}

fn owner_name_is_self(unique_name: Option<&str>, owner: &str) -> bool {
    // Unique names come from the live connection and must match the queried owner exactly
    unique_name == Some(owner)
}

#[cfg(test)]
#[path = "tests/dbus_owner.rs"]
mod tests;
