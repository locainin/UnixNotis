//! Bus-name acquisition and client ownership lifecycle

mod clients;
mod health;
mod names;
mod ownership;

pub use health::{monitor_required_bus_names, verify_name_owner};
pub use names::{log_name_reply, request_control_name, request_well_known_name};
pub use ownership::{spawn_client_owner_watch, wait_for_owner_state};

#[cfg(test)]
mod tests;
