//! Image handling for notification hints and icon metadata
//!
//! The public types live here so callers keep importing `NotificationImage`
//! and `ImageData` from the same place. Parsing, projection, validation, and
//! RGB expansion live in focused files under `model/image`

use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

#[path = "image/hints.rs"]
mod hints;
#[path = "image/normalize.rs"]
mod normalize;
#[path = "image/projection.rs"]
mod projection;
#[path = "image/rgb.rs"]
mod rgb;

/// Raw image data payload from notification hints.
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

/// Image information derived from standard hints and app_icon.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Default, PartialEq, Eq)]
pub struct NotificationImage {
    pub has_image_data: bool,
    pub image_data: ImageData,
    pub image_path: String,
    pub icon_name: String,
}

// Bound untrusted image payloads to keep daemon/UI memory predictable under floods
pub(super) const MAX_IMAGE_BYTES: usize = 256 * 1024;
pub(super) const MAX_IMAGE_DIMENSION: i32 = 256;
pub(super) const MAX_IMAGE_PATH_BYTES: usize = 1024;
pub(super) const MAX_ICON_NAME_BYTES: usize = 256;

#[cfg(test)]
use hints::{owned_to_string, strip_desktop_suffix};
#[cfg(test)]
use rgb::expand_rgb_row_scalar;
#[cfg(all(test, target_arch = "x86_64"))]
use rgb::expand_rgb_row_ssse3;

#[cfg(test)]
#[path = "tests/image/index.rs"]
mod tests;
