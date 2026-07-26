//! Inhibitor bookkeeping and suppression state

mod api;
mod model;

pub(in crate::store) use model::Inhibitor;

#[cfg(test)]
mod tests;
