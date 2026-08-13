//! D-Bus server for org.freedesktop.Notifications

mod flow_control;
pub(in crate::daemon) mod identity;
mod ingress;
mod server;

pub(in crate::daemon) use flow_control::{
    notification_signal_mode_for_sender, NotificationBurstState, NotificationSignalMode,
};
pub use identity::DesktopIndexSnapshot;
pub(in crate::daemon) use identity::SenderMetadataCache;
pub use identity::{spawn_desktop_index_refresh, DesktopIdentityIndex};
pub use server::NotificationIngress;
pub use server::NotificationServer;
