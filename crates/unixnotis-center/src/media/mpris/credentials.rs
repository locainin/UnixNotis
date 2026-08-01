//! Process credentials for MPRIS owner checks

#[cfg(target_os = "linux")]
use zbus::names::BusName;
#[cfg(target_os = "linux")]
use zbus::zvariant::{DeserializeDict, Type};
#[cfg(target_os = "linux")]
use zbus::{proxy, Connection};

#[cfg(target_os = "linux")]
#[derive(Debug, Default, DeserializeDict, Type)]
#[zvariant(signature = "a{sv}")]
pub(super) struct MprisCredentials {
    #[zvariant(rename = "ProcessFD")]
    pub(super) process_fd: Option<zbus::zvariant::OwnedFd>,
    #[zvariant(rename = "ProcessID")]
    pub(super) process_id: Option<u32>,
}

#[cfg(target_os = "linux")]
#[proxy(
    interface = "org.freedesktop.DBus",
    default_service = "org.freedesktop.DBus",
    default_path = "/org/freedesktop/DBus"
)]
trait ConnectionCredentialsDbus {
    fn get_connection_credentials(&self, bus_name: BusName<'_>) -> zbus::Result<MprisCredentials>;
}

#[cfg(target_os = "linux")]
pub(super) async fn get_connection_credentials(
    connection: &Connection,
    bus_name: BusName<'_>,
) -> Option<MprisCredentials> {
    let proxy = ConnectionCredentialsDbusProxy::new(connection).await.ok()?;
    proxy.get_connection_credentials(bus_name).await.ok()
}
