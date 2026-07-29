//! Stable D-Bus names and inhibitor scopes

/// Well-known bus name for the `UnixNotis` control interface
pub const CONTROL_BUS_NAME: &str = "com.unixnotis.Control";
/// Object path for control methods and signals
pub const CONTROL_OBJECT_PATH: &str = "/com/unixnotis/Control";
/// D-Bus interface name for control calls
pub const CONTROL_INTERFACE: &str = "com.unixnotis.Control";
/// Coordinated private interface version shared by daemon and UI binaries
pub const CONTROL_API_VERSION: u32 = 2;
/// Freedesktop notification service name owned by the active notification daemon
pub const NOTIFICATIONS_BUS_NAME: &str = "org.freedesktop.Notifications";
/// Inhibit scope meaning all notification output
pub const INHIBIT_SCOPE_ALL: u32 = 0;
/// Inhibit scope bitmask value for suppressing popups
pub const INHIBIT_SCOPE_POPUPS: u32 = 1;
