//! D-Bus server implementation and daemon state coordination

mod auth;
mod bus;
mod control;
mod errors;
mod events;
mod notifications;
mod state;

pub use bus::{
    log_current_owner, log_name_reply, request_control_name, request_well_known_name,
    spawn_client_owner_watch, wait_for_owner_state,
};
pub use control::ControlServer;
pub use errors::to_fdo_error;
pub use notifications::NotificationServer;
pub(in crate::daemon) use notifications::NotificationSignalMode;
pub use state::DaemonState;

pub const NOTIFICATIONS_OBJECT_PATH: &str = "/org/freedesktop/Notifications";
