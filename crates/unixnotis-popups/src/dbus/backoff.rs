//! Popup aliases for the shared reconnect policy

pub use unixnotis_core::reconnect::{
    Backoff, RetryLog, BACKOFF_BASE_MS, BACKOFF_MAX_MS, RETRY_WARN_INTERVAL_SECS,
};

#[cfg(test)]
#[path = "tests/backoff.rs"]
mod tests;
