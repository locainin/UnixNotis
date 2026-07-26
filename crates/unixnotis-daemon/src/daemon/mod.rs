//! D-Bus server implementation and daemon state coordination

mod auth;
mod bus;
mod control;
mod errors;
mod events;
mod notifications;
mod state;

pub use bus::{
    log_name_reply, monitor_required_bus_names, request_control_name, request_well_known_name,
    spawn_client_owner_watch, verify_name_owner, wait_for_owner_state,
};
pub use control::ControlServer;
pub use errors::to_fdo_error;
pub use notifications::DesktopIndexSnapshot;
pub use notifications::NotificationIngress;
pub use notifications::NotificationServer;
pub(in crate::daemon) use notifications::NotificationSignalMode;
pub use notifications::{spawn_desktop_index_refresh, DesktopIdentityIndex};
pub use state::DaemonState;

pub const NOTIFICATIONS_OBJECT_PATH: &str = "/org/freedesktop/Notifications";
