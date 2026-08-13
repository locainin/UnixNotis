//! Notification image hint parsing, normalization, projection, and RGB expansion

mod hints;
mod model;
mod normalize;
mod projection;
mod rgb;

pub use model::{ImageData, NotificationImage, NotificationVisualRole};
pub(super) use model::{MAX_IMAGE_BYTES, MAX_IMAGE_DIMENSION};

#[cfg(test)]
mod tests;
