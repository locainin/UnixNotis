//! Installer workflow modules

mod build_accel;
mod controller;
mod events;
mod recovery;
mod worker;

pub(super) use build_accel::{handle_build_accel_enter, prepare_build_accel_prompt};
pub(super) use controller::start_action;
pub(super) use events::{apply_worker_event, reset_to_menu};
#[cfg(test)]
mod tests;
