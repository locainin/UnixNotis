//! Shared daemon state and signal fanout coordination

mod cache;
mod dnd;
mod model;
mod notifications;
mod runtime;
mod scheduler;
mod signals;

pub use model::DaemonState;

#[cfg(test)]
mod tests;
