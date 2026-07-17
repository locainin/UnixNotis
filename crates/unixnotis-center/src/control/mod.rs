//! `UnixNotis` control client and UI event projection

mod backoff;
mod client;
mod commands;
mod events;
mod model;
mod reconnect;
mod seed;
mod subscriptions;

pub use client::start_control_task;
pub use model::{UiCommand, UiEvent};
