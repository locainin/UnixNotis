//! Configuration module wiring for UnixNotis.
//!
//! Keeps config types, I/O, and runtime defaults in separate files.

mod config_commands;
mod config_io;
mod config_layout;
mod config_media;
mod config_rules;
mod config_runtime;
mod config_theme;
mod config_types;
mod config_widgets;

pub use config_io::{ConfigError, ThemePaths};
pub use config_layout::*;
pub use config_media::*;
pub use config_rules::*;
pub use config_theme::*;
pub use config_types::*;
pub use config_widgets::*;
