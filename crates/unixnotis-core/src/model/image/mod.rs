//! Notification image hint parsing, normalization, projection, and RGB expansion

mod hints;
mod model;
mod normalize;
mod projection;
mod rgb;

pub use model::{ImageData, NotificationImage};
pub(super) use model::{
    MAX_ICON_NAME_BYTES, MAX_IMAGE_BYTES, MAX_IMAGE_DIMENSION, MAX_IMAGE_PATH_BYTES,
};

#[cfg(test)]
pub(super) use hints::{owned_to_string, strip_desktop_suffix};
#[cfg(test)]
pub(super) use rgb::expand_rgb_row_scalar;
#[cfg(all(test, target_arch = "x86_64"))]
pub(super) use rgb::expand_rgb_row_ssse3;

#[cfg(test)]
mod tests;
