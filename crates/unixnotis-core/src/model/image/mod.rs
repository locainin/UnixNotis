//! Notification image hint parsing, normalization, projection, and RGB expansion

mod hints;
mod model;
mod normalize;
mod projection;
mod rgb;

pub use model::{ImageData, NotificationImage, NotificationVisualRole};
pub(super) use model::{
    MAX_ICON_NAME_BYTES, MAX_IMAGE_BYTES, MAX_IMAGE_DIMENSION, MAX_IMAGE_PATH_BYTES,
};

#[cfg(test)]
mod tests;
