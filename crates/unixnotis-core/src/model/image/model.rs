//! Public notification image records and shared limits
//!
//! The public types live here so callers keep importing `NotificationImage`
//! and `ImageData` from the same place. Parsing, projection, validation, and
//! RGB expansion live in focused files under `model/image`

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use zbus::zvariant::Type;

/// Raw image data payload from notification hints
#[derive(Debug, Clone, Serialize, Deserialize, Type, Default, PartialEq, Eq)]
pub struct ImageData {
    pub width: i32,
    pub height: i32,
    pub rowstride: i32,
    pub has_alpha: bool,
    pub bits_per_sample: i32,
    pub channels: i32,
    pub data: Vec<u8>,
}

/// Presentation role selected by the daemon after attribution and payload checks
#[derive(Debug, Copy, Clone, Serialize_repr, Deserialize_repr, Type, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum NotificationVisualRole {
    #[default]
    None = 0,
    ConversationAvatar = 1,
}

/// Image information derived from standard hints and `app_icon`
#[derive(Debug, Clone, Serialize, Deserialize, Type, Default, PartialEq, Eq)]
pub struct NotificationImage {
    pub has_image_data: bool,
    pub image_data: ImageData,
    // Conversation avatars are decoded before leaving the daemon
    pub visual_role: NotificationVisualRole,
    pub conversation_avatar: ImageData,
    // image_path remains reserved for message content media
    pub image_path: String,
    pub icon_name: String,
}

// Bound untrusted image payloads to keep daemon/UI memory predictable under floods
pub(in crate::model) const MAX_IMAGE_BYTES: usize = 256 * 1024;
pub(in crate::model) const MAX_IMAGE_DIMENSION: i32 = 256;
pub(in crate::model) const MAX_IMAGE_PATH_BYTES: usize = 1024;
pub(in crate::model) const MAX_ICON_NAME_BYTES: usize = 256;
