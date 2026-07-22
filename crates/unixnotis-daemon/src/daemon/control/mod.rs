//! D-Bus server for com.unixnotis.Control

mod action;
mod clear;
mod dnd;
mod inhibit;
mod panel;
mod query;
mod reply;
mod sanitize;
mod server;
mod watch;

pub use server::ControlServer;
pub use watch::spawn_inhibitor_owner_watch;

// Cap inhibitor count so memory use stays bounded even under abusive clients
const MAX_ACTIVE_INHIBITORS: u32 = 128;

#[cfg(test)]
mod tests;
