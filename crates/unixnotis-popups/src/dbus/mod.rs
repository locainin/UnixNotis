//! D-Bus runtime for popup UI events and control updates

// Submodules keep retry policy, seeding, and runtime wiring out of the
// public entrypoint so this file stays easy to scan
mod backoff;
mod commands;
mod runtime;
mod seed;
mod types;

pub use runtime::start_dbus_runtime;
pub use types::{UiCommand, UiEvent};
