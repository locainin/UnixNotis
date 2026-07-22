//! D-Bus server for org.freedesktop.Notifications

mod limits;
mod payload;
mod quota;
mod sender;
pub(in crate::daemon) mod sender_cache;
mod server;

pub use server::NotificationServer;
