//! Bus-name acquisition and client ownership lifecycle

mod clients;
mod names;
mod ownership;

pub use names::{log_name_reply, request_control_name, request_well_known_name};
pub use ownership::{log_current_owner, spawn_client_owner_watch, wait_for_owner_state};

#[cfg(test)]
mod tests;
