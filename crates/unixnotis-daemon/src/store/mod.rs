//! Notification store with ordering, history, and suppression policies

mod dnd;
mod inhibitors;
mod model;
mod notifications;
mod runtime;

pub use model::{DismissOutcome, DndWrite, InsertOutcome, NotificationStore};

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
