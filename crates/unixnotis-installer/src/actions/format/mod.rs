//! Formatting helpers for installer-facing status output

mod daemon_status;

pub use daemon_status::{
    daemon_has_displayable_status, daemon_status_is_warning, format_daemon_status, summarize_owner,
};

#[cfg(test)]
mod tests;
