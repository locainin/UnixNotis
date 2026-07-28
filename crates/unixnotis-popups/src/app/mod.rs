//! Popup startup, reload, and event-loop module wiring

mod command;
mod reload;
pub mod resources;
mod runtime;
mod startup;

pub use command::{run, Args};

#[cfg(test)]
mod tests;
