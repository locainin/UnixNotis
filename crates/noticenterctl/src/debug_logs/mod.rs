//! Journalctl follower used when the panel opens in debug mode

mod command;
mod journal;

pub(crate) use command::follow_debug_logs;

#[cfg(test)]
mod tests;
