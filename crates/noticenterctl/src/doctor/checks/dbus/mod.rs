//! Bounded session-bus and `UnixNotis` control inspection

mod classify;
mod control;
mod owners;
mod session;

use std::time::Duration;

use zbus::fdo::DBusProxy;
use zbus::Connection;

use super::super::report::DoctorCheck;

pub(super) const DBUS_CHECK_TIMEOUT: Duration = Duration::from_secs(3);

pub(in crate::doctor) struct DoctorBusResult {
    pub checks: Vec<DoctorCheck>,
    pub control_owned: bool,
    pub connected: bool,
}

pub(in crate::doctor) async fn inspect_bus() -> DoctorBusResult {
    let session::SessionProbe { connection, checks } = session::probe_session().await;
    let Some(connection) = connection else {
        return DoctorBusResult {
            checks,
            control_owned: false,
            connected: false,
        };
    };
    debug_assert!(
        checks.is_empty(),
        "session probing must not report checks when a connection is available"
    );
    inspect_bus_connection(&connection).await
}

pub(super) async fn inspect_bus_connection(connection: &Connection) -> DoctorBusResult {
    let checks = session::connected_checks(connection).await;
    inspect_connected_bus(connection, checks).await
}

async fn inspect_connected_bus(
    connection: &Connection,
    mut checks: Vec<DoctorCheck>,
) -> DoctorBusResult {
    let proxy = match session::build_bus_proxy(connection).await {
        Ok(proxy) => proxy,
        Err(check) => {
            checks.push(check);
            return DoctorBusResult {
                checks,
                control_owned: false,
                connected: true,
            };
        }
    };
    inspect_owners_and_control(connection, &proxy, &mut checks).await
}

async fn inspect_owners_and_control(
    connection: &Connection,
    proxy: &DBusProxy<'_>,
    checks: &mut Vec<DoctorCheck>,
) -> DoctorBusResult {
    let notifications = owners::probe_notifications_owner(proxy).await;
    let notification_owner = notifications.owner().map(ToOwned::to_owned);
    checks.push(notifications.check);
    if notification_owner.is_none() {
        checks.push(owners::notification_readiness_failure());
    }

    let control_probe = owners::probe_control_owner(proxy).await;
    let control_owned = control_probe.owner().is_some();
    let control_owner_name = control_probe.owner().map(ToOwned::to_owned);
    checks.push(control_probe.check);
    if let (Some(notification_owner), Some(control_owner_name)) =
        (&notification_owner, &control_owner_name)
    {
        checks.push(owners::shared_owner_check(
            notification_owner,
            control_owner_name,
        ));
    }

    if control_owned {
        checks.extend(control::inspect_control(connection).await);
    } else {
        checks.push(control::unavailable_control_check());
    }

    DoctorBusResult {
        checks: std::mem::take(checks),
        control_owned,
        connected: true,
    }
}

#[cfg(test)]
mod tests;
