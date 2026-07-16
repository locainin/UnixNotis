//! Command parsing and local or D-Bus command dispatch

mod local;
mod runner;

#[cfg(test)]
use local::handle_local_command;
pub use runner::run;

#[cfg(test)]
#[path = "tests/runner.rs"]
mod tests;
