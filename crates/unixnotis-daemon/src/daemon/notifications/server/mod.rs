//! Freedesktop notification D-Bus server and request handling

mod capabilities;
mod close;
mod flow;
mod interface;

pub use interface::NotificationServer;

#[cfg(test)]
#[path = "tests/interface.rs"]
mod tests;
