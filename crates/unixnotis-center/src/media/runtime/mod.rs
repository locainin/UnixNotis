//! Media runtime module wiring

mod cache;
mod dispatch;
mod r#loop;
mod owner;
mod refresh;
mod schedule;
mod signal;
mod snapshot;
mod startup;
mod state;

pub(super) use signal::{MediaRefreshOrigin, MediaSignal};
pub(super) use startup::start_media_task;

pub(super) const MEDIA_SIGNAL_CAPACITY: usize = 256;

#[cfg(test)]
mod tests;
