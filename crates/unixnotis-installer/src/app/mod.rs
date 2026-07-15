//! Installer application state and event-loop ownership

pub mod events;
pub mod handlers;
pub mod runtime;
mod state;
pub mod workflow;

pub use events::ExitAction;
pub use state::{App, BuildAccelMenuMode, BuildAccelState, MenuItem, ProgressState, Screen};
