//! Notification store with ordering, history, and suppression policies

mod dnd;
mod inhibitors;
mod model;
mod notifications;
mod runtime;

// Internal store primitives used by the main NotificationStore type
use dnd::{DndStateStore, DND_STATE_VERSION};
use inhibitors::Inhibitor;
pub use model::{DismissOutcome, DndWrite, InsertOutcome, NotificationStore};
use notifications::HistoryStore;

#[cfg(test)]
mod test_support;
