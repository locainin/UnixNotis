//! Shared daemon state and signal fanout coordination

mod model;
mod notification_lifecycle;
mod schedulers;
mod status;

pub use model::DaemonState;

#[cfg(test)]
mod tests;
