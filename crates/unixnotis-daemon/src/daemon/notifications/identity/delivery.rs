//! Reconnect-safe callback destination resolution

use zbus::fdo::DBusProxy;
use zbus::names::{BusName, UniqueName};
use zbus::Connection;

use super::{read_process_start_time, SenderMetadataCache, SENDER_CREDENTIAL_TIMEOUT};

pub(in crate::daemon) async fn resolve_callback_destination(
    cache: &SenderMetadataCache,
    connection: &Connection,
    retained_bus_name: Option<&str>,
    pid: Option<u32>,
    start_time: Option<u64>,
) -> Option<BusName<'static>> {
    let proxy = DBusProxy::new(connection).await.ok()?;

    if let (Some(pid), Some(start_time)) = (pid, start_time) {
        // Stable lifetime evidence permits reconnect-safe address rebinding
        if let Some(retained) = retained_bus_name {
            if let Some(destination) = verified_destination(&proxy, retained, pid, start_time).await
            {
                return Some(destination);
            }
        }

        // A unique bus name is an ephemeral delivery address, not ownership
        // Every cached address is verified before callback delivery
        for current in cache.sender_candidates_for_process(pid, start_time, retained_bus_name) {
            if let Some(destination) = verified_destination(&proxy, &current, pid, start_time).await
            {
                return Some(destination);
            }
        }
        return None;
    }

    // Weak evidence may retain one exact live address but can never authorize rebinding
    let retained = retained_bus_name?;
    let bus_name = BusName::try_from(retained).ok()?.to_owned();
    tokio::time::timeout(
        SENDER_CREDENTIAL_TIMEOUT,
        proxy.name_has_owner(bus_name.clone()),
    )
    .await
    .ok()?
    .ok()?
    .then_some(bus_name)
}

async fn verified_destination(
    proxy: &DBusProxy<'_>,
    candidate: &str,
    expected_pid: u32,
    expected_start_time: u64,
) -> Option<BusName<'static>> {
    let unique_name = UniqueName::try_from(candidate).ok()?.to_owned();
    let start_before = read_process_start_time(expected_pid);
    let bus_pid = tokio::time::timeout(
        SENDER_CREDENTIAL_TIMEOUT,
        proxy.get_connection_unix_process_id(unique_name.clone().into()),
    )
    .await
    .ok()?
    .ok()?;
    let start_after = read_process_start_time(expected_pid);
    if !credentials_match_lifetime(
        bus_pid,
        expected_pid,
        start_before,
        start_after,
        expected_start_time,
    ) {
        return None;
    }
    Some(unique_name.into())
}

fn credentials_match_lifetime(
    bus_pid: u32,
    expected_pid: u32,
    start_before: Option<u64>,
    start_after: Option<u64>,
    expected_start_time: u64,
) -> bool {
    // Both samples must identify the retained lifetime so PID reuse cannot race delivery
    bus_pid == expected_pid
        && start_before == Some(expected_start_time)
        && start_after == Some(expected_start_time)
}

#[cfg(test)]
#[path = "tests/delivery.rs"]
mod tests;
