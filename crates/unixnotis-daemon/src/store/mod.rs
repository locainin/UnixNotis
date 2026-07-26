//! Notification store with ordering, history, and suppression policies

// Focused modules keep policy and lifecycle logic isolated and easier to test
mod core;
mod dnd;
mod inhibit;
mod inhibitor_api;
mod notifications;
mod state;
mod types;

// Internal store primitives used by the main NotificationStore type
use inhibit::Inhibitor;
use notifications::HistoryStore;
use state::{DndStateStore, DND_STATE_VERSION};
pub use types::DndWrite;
pub use types::{DismissOutcome, InsertOutcome, NotificationStore};

#[cfg(test)]
mod tests;
