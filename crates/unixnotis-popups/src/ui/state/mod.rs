//! Popup UI state model, construction, and top-level event handling

mod constructor;
mod events;
mod model;

pub use model::UiState;
pub(super) use model::{IconCacheEntry, IconResolutionKey};

#[cfg(test)]
mod tests;
