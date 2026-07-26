//! Do-not-disturb state changes and persistence

mod persistence;
mod state;

pub(in crate::store) use persistence::{DndStateStore, DND_STATE_VERSION};

#[cfg(test)]
pub(in crate::store) use persistence::{PersistedDndState, DND_STATE_FILE};

#[cfg(test)]
mod tests;
