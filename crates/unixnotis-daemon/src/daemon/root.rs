//! D-Bus server implementation and daemon state coordination

mod auth;
mod bus_names;
mod control;
mod errors;
mod notifications;
mod signal_burst;
mod state;

pub use bus_names::{log_name_reply, request_control_name, request_well_known_name};
pub use control::{spawn_inhibitor_owner_watch, ControlServer};
pub(crate) use errors::to_fdo_error;
pub use notifications::NotificationServer;
pub(in crate::daemon) use signal_burst::NotificationSignalMode;
pub use state::DaemonState;

pub(crate) const NOTIFICATIONS_OBJECT_PATH: &str = "/org/freedesktop/Notifications";
