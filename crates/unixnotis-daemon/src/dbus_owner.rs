//! D-Bus owner tracking helpers.
//!
//! Provides reusable helpers for name ownership checks during startup and trial mode.

use std::time::Duration;

use anyhow::Result;
use futures_util::StreamExt;
use tracing::info;
use zbus::fdo::DBusProxy;
use zbus::Connection;

pub(super) async fn wait_for_owner_state(
    dbus_proxy: &DBusProxy<'_>,
    name: zbus::names::BusName<'_>,
    expect_owner: bool,
    timeout: Duration,
) -> Result<bool> {
    let has_owner = dbus_proxy
        .name_has_owner(name.clone())
        .await
        .unwrap_or(false);
    if has_owner == expect_owner {
        return Ok(true);
    }

    let name_str = name.to_string();
    let mut stream = dbus_proxy
        .receive_name_owner_changed_with_args(&[(0, name_str.as_str())])
        .await?;
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
                let has_owner = !new_owner.is_empty();
                if has_owner == expect_owner {
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
    let is_self = unique_name.as_deref() == Some(owner.as_str());
    if is_self {
        info!(owner, "org.freedesktop.Notifications owner (self)");
    } else {
        info!(owner, "org.freedesktop.Notifications owner");
    }
    Ok(is_self)
}
