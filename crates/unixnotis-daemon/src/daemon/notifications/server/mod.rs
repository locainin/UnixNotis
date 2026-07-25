//! Freedesktop notification D-Bus server and request handling

mod capabilities;
mod close;
mod flow;
mod ingress;
mod interface;
mod notify_body;

pub use ingress::NotificationIngress;
pub use interface::NotificationServer;

#[cfg(test)]
#[path = "tests/interface.rs"]
mod tests;
