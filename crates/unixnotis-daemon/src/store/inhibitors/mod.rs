//! Inhibitor bookkeeping and suppression state

mod api;
mod model;

use super::NotificationStore;
pub(in crate::store) use model::Inhibitor;

#[cfg(test)]
mod tests;
