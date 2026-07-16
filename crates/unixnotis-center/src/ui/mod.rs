//! Center UI state, widget wiring, and event-handling modules

mod command;
mod config;
mod config_media;
mod events;
mod hyprland;
mod icons;
// Startup wiring remains a child module so constructors can access private UI parts
mod init;
mod input_guard;
mod list;
mod marquee;
mod media_art;
mod media_widget;
mod panel;
mod perf_probe;
mod refresh;
mod reload_notice;
mod state;
mod visibility;
mod widget_builders;
mod widgets;

pub use command::try_send_command;
pub use state::{UiState, UiStateInit};
