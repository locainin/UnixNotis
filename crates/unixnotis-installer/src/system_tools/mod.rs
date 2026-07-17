//! Trusted external tool lookup for installer probes

mod command;
mod lookup;

// Fake executable routing lives under /tests and never enters production binaries
#[expect(
    clippy::cfg_not_test,
    reason = "production routing must not compile beside its test double"
)]
#[cfg(not(test))]
mod routing;
#[cfg(test)]
#[path = "tests/routing.rs"]
pub mod routing;

pub use command::{command, program_exists, program_path};

#[cfg(test)]
mod tests;
