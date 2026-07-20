//! Trusted external tool lookup for local diagnostic helpers

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

pub use command::{command, command_from_spec, tokio_command_from_spec, trusted_program_path};

#[cfg(test)]
mod tests;
