//! Center UI state, widget wiring, and event-handling modules

mod command;
mod events;
mod hyprland;
mod icons;
mod local_file;
mod reload;
// Startup wiring remains a child module so constructors can access private UI parts
mod init;

mod media;
mod motion;
mod notifications;
mod panel;
mod state;

mod widget_builders;
mod widgets;

pub use command::try_send_command;
pub use state::{UiState, UiStateInit};
