//! Formatting helpers for installer-facing status output

mod daemon_status;

pub use daemon_status::{format_daemon_status, summarize_owner};

#[cfg(test)]
mod tests;
