//! Public media handles and UI-facing value types

mod handle;
mod model;

pub use super::art::{MediaArtKey, MediaArtSource};
pub use handle::{start_media_task, MediaHandle};
pub use model::{MediaCommand, MediaInfo};

#[cfg(test)]
mod tests;
