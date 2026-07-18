//! D-Bus caller credentials used by authorization checks

use zbus::names::BusName;
use zbus::zvariant::{DeserializeDict, Type};
use zbus::{proxy, Connection};

use crate::daemon::to_fdo_error;

/// Credentials returned together by the message bus
///
/// zbus 4 omits the optional `ProcessFD` field from its built-in type, so this
/// local view keeps that stable process handle without changing the wire format
#[derive(Debug, Default, DeserializeDict, Type)]
#[zvariant(signature = "a{sv}")]
pub(super) struct CallerCredentials {
    #[zvariant(rename = "UnixUserID")]
    pub(super) unix_user_id: Option<u32>,
    #[cfg(unix)]
    #[zvariant(rename = "ProcessFD")]
    pub(super) process_fd: Option<zbus::zvariant::OwnedFd>,
    #[zvariant(rename = "ProcessID")]
    pub(super) process_id: Option<u32>,
}

impl CallerCredentials {
    pub(super) const fn unix_user_id(&self) -> Option<u32> {
        self.unix_user_id
    }

    #[cfg(unix)]
    pub(super) const fn process_fd(&self) -> Option<&zbus::zvariant::OwnedFd> {
        self.process_fd.as_ref()
    }

    pub(super) const fn process_id(&self) -> Option<u32> {
        self.process_id
    }
}

#[proxy(
    interface = "org.freedesktop.DBus",
    default_service = "org.freedesktop.DBus",
    default_path = "/org/freedesktop/DBus"
)]
trait ConnectionCredentialDbus {
    /// Return all available credentials in one bus reply
    fn get_connection_credentials(&self, bus_name: BusName<'_>) -> zbus::Result<CallerCredentials>;
}

pub(super) async fn connection_credentials(
    connection: &Connection,
    bus_name: BusName<'_>,
) -> zbus::fdo::Result<CallerCredentials> {
    // One reply keeps the uid, pid, and process handle from the same bus snapshot
    let proxy = ConnectionCredentialDbusProxy::new(connection)
        .await
        .map_err(to_fdo_error)?;
    proxy
        .get_connection_credentials(bus_name)
        .await
        .map_err(to_fdo_error)
}
