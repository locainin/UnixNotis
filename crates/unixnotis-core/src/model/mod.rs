//! Notification data model and image hint parsing

// Keep the public model surface small by splitting large helpers into files.
mod attribution;
mod diagnostics;
mod image;
mod notification;
mod reply;
mod types;

// Re-export the public surface so callers continue to import from unixnotis_core::model.
pub use attribution::{
    ApplicationActionPolicy, AttributionClass, InlineReplyPolicy, NotificationAttribution,
};
pub use diagnostics::{
    AttributionDiagnostics, CommandLineQualityView, LaunchAuthorityView, LaunchVerificationView,
    RecordTrust,
};
pub use image::{ImageData, NotificationImage};
pub use notification::{Notification, NotificationKey, NotificationView};
pub use reply::InlineReply;
pub use types::{Action, Urgency};
