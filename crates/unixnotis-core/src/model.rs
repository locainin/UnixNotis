//! Notification data model and image hint parsing.

// Keep the public model surface small by splitting large helpers into files.
mod model_image;
mod model_notification;
mod model_types;

// Re-export the public surface so callers continue to import from unixnotis_core::model.
pub use model_image::{ImageData, NotificationImage};
pub use model_notification::{Notification, NotificationView};
pub use model_types::{Action, Urgency};
