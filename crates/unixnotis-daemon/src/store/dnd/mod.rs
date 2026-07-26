//! Do-not-disturb state changes and persistence

pub(in crate::store) mod persistence;
mod state;

pub(in crate::store) use persistence::{DndStateStore, DND_STATE_VERSION};

#[cfg(test)]
mod tests;
