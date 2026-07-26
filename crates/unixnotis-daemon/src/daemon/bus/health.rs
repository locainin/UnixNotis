//! Session-bus identity, ownership verification, and runtime health checks

use std::time::Duration;

use anyhow::{ensure, Context, Result};
use unixnotis_core::{CONTROL_BUS_NAME, NOTIFICATIONS_BUS_NAME};
use zbus::fdo::DBusProxy;
use zbus::names::BusName;
use zbus::Connection;

const BUS_HEALTH_INTERVAL: Duration = Duration::from_secs(1);
const BUS_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

pub async fn verify_name_owner(
    dbus: &DBusProxy<'_>,
    connection: &Connection,
    name: &'static str,
) -> Result<()> {
    let expected = connection
        .unique_name()
        .context("session bus did not assign a unique name")?;
    let bus_name = BusName::try_from(name).context("invalid required D-Bus name")?;
    let actual = tokio::time::timeout(BUS_PROBE_TIMEOUT, dbus.get_name_owner(bus_name))
        .await
        .with_context(|| format!("D-Bus owner probe timed out for {name}"))?
        .with_context(|| format!("D-Bus owner probe failed for {name}"))?;

    ensure!(
        actual.as_str() == expected.as_str(),
        "{name} owner mismatch: expected {expected}, found {actual}"
    );
    Ok(())
}

pub async fn monitor_required_bus_names(connection: Connection) -> Result<()> {
    let dbus = DBusProxy::new(&connection)
        .await
        .context("create D-Bus health proxy")?;

    loop {
        tokio::time::sleep(BUS_HEALTH_INTERVAL).await;
        for required in [NOTIFICATIONS_BUS_NAME, CONTROL_BUS_NAME] {
            verify_name_owner(&dbus, &connection, required).await?;
        }
    }
}
