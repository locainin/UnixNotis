//! Center UI state, widget wiring, and event-handling modules

mod command;
mod events;
mod hyprland;
mod icons;
mod reload;
// Startup wiring remains a child module so constructors can access private UI parts
mod init;
mod input_guard;
mod media;
mod notifications;
mod panel;
mod perf_probe;
mod state;
mod visibility;
mod widget_builders;
mod widgets;

pub use command::try_send_command;
pub use state::{UiState, UiStateInit};
