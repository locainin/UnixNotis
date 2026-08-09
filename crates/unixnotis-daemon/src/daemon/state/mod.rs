//! Shared daemon state and signal fanout coordination

mod interaction_gates;
mod model;
mod notification_commit;
mod notification_lifecycle;
mod schedulers;
mod status;

pub(in crate::daemon) use interaction_gates::InteractionGates;
pub use model::DaemonState;

#[cfg(test)]
mod tests;
