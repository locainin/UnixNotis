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
    ApplicationProvidedIcon = 2,
    ContentImage = 3,
}

/// Pixel visuals retained after daemon-side validation
#[derive(Debug, Clone, Serialize, Deserialize, Type, Default, PartialEq, Eq)]
pub struct NotificationImage {
    /// Desktop-index-selected identity icon
    pub badge_icon: String,
    /// Sender-supplied theme name retained only as a decorative lookup hint
    #[serde(default)]
    pub claimed_theme_icon: String,
    /// Sender-supplied desktop id retained only for bounded decorative lookup
    /// This value is never attribution evidence or an authorization input
    #[serde(default)]
    pub claimed_desktop_id: String,
    /// Safely decoded sender-provided visual
    pub sender_visual_role: NotificationVisualRole,
    pub sender_visual: ImageData,
    /// Safely decoded message content image
    pub content_image: ImageData,
}

// Bound untrusted image payloads to keep daemon/UI memory predictable under floods
pub(in crate::model) const MAX_IMAGE_BYTES: usize = 256 * 1024;
pub(in crate::model) const MAX_IMAGE_DIMENSION: i32 = 256;
