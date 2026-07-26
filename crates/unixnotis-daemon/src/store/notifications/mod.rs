//! Active notification lifecycle, history, ownership, and rule policy

mod history;
mod insertion;
mod lifecycle;
mod ownership;
pub(super) mod rules;

use super::{DismissOutcome, InsertOutcome, NotificationStore};
pub(super) use history::HistoryStore;

#[cfg(test)]
mod tests;
