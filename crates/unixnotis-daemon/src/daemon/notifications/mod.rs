//! D-Bus server for org.freedesktop.Notifications

mod limits;
mod payload;
mod sender;
mod server;

pub use server::NotificationServer;
