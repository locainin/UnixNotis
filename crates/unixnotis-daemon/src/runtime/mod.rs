//! Daemon execution, shutdown, and trial-cleanup module wiring

mod daemon;
mod runner;
mod shutdown;
mod trial_cleanup;

pub use runner::run;

#[cfg(test)]
mod tests;
