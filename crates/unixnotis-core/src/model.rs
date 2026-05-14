//! Notification data model and image hint parsing.

// Keep the public model surface small by splitting large helpers into files.
mod image;
mod notification;
mod types;

// Re-export the public surface so callers continue to import from unixnotis_core::model.
pub use image::{ImageData, NotificationImage};
pub use notification::{Notification, NotificationView};
pub use types::{Action, Urgency};
