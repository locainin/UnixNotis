//! Journalctl follower used when the panel opens in debug mode

mod command;
pub mod journal;

pub use command::follow_debug_logs;

#[cfg(test)]
mod tests;
