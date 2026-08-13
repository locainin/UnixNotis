//! Freedesktop notification client proxy contract

use std::collections::HashMap;

use zbus::proxy;
use zbus::zvariant::OwnedValue;

#[proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
pub trait Notifications {
    /// Capabilities advertised by the active notification server
    fn get_capabilities(&self) -> zbus::Result<Vec<String>>;

    /// Stable server identity and protocol version
    fn get_server_information(&self) -> zbus::Result<(String, String, String, String)>;

    /// Submit one notification and return its assigned identifier
    #[expect(
        clippy::too_many_arguments,
        reason = "the D-Bus method must match the freedesktop notification protocol"
    )]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;
}
