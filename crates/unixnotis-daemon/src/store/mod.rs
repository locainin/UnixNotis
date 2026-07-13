//! Notification store with ordering, history, and suppression policies

// Focused modules keep policy and lifecycle logic isolated and easier to test
mod core;
mod dnd;
mod history;
mod identity;
mod inhibit;
mod inhibitor_api;
mod lifecycle;
mod rules;
mod state;
mod types;

// Internal store primitives used by the main NotificationStore type
use history::HistoryStore;
use inhibit::Inhibitor;
use state::{DndStateStore, DND_STATE_VERSION};
pub use types::DndWrite;
pub use types::{DismissOutcome, InsertOutcome, NotificationStore};

#[cfg(test)]
use rules::contains_ci;

#[cfg(test)]
mod tests;
