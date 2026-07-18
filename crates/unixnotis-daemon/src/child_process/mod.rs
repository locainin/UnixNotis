//! Child process management for UI components

mod paths;
mod process;
mod supervisor;

pub use process::{spawn_center_supervisor, spawn_popups_supervisor};
use process::{RestartBackoff, UiProcessKind};
