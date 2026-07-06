//! D-Bus server implementation and daemon state coordination

#[path = "auth.rs"]
mod auth;
#[path = "bus_names.rs"]
mod bus_names;
#[path = "control/mod.rs"]
mod control;
#[path = "errors.rs"]
mod errors;
#[path = "notifications/mod.rs"]
mod notifications;
#[path = "signal_burst.rs"]
mod signal_burst;
#[path = "state/mod.rs"]
mod state;

pub use bus_names::{log_name_reply, request_control_name, request_well_known_name};
pub(crate) use control::spawn_inhibitor_owner_watch;
pub use control::ControlServer;
pub(crate) use errors::to_fdo_error;
pub use notifications::NotificationServer;
pub(in crate::daemon) use signal_burst::NotificationSignalMode;
pub use state::DaemonState;

pub(crate) const NOTIFICATIONS_OBJECT_PATH: &str = "/org/freedesktop/Notifications";
