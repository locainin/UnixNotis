//! Popup UI state model, construction, and top-level event handling

mod constructor;
mod events;
mod model;

pub(super) use model::IconCacheEntry;
pub use model::UiState;

#[cfg(test)]
mod tests;
