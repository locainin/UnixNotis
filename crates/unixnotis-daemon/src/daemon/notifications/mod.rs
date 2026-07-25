//! D-Bus server for org.freedesktop.Notifications

mod flow_control;
pub(in crate::daemon) mod identity;
mod limits;
mod metrics;
mod payload;
mod quota;
mod sender;
pub(in crate::daemon) mod sender_cache;
mod server;

pub(in crate::daemon) use flow_control::{
    notification_signal_mode_for_sender, NotificationBurstState, NotificationSignalMode,
};
pub use server::NotificationIngress;
pub use server::NotificationServer;
