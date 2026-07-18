//! Command parsing and local or D-Bus command dispatch

mod local;
mod runner;

pub use runner::run;

#[cfg(test)]
#[path = "tests/runner.rs"]
mod tests;
