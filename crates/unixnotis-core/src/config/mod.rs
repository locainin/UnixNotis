//! Configuration module wiring for UnixNotis.
//!
//! Keeps config types, I/O, and runtime cleanup in separate files

mod commands;
mod io;
mod layout;
mod media;
mod rules;
mod runtime;
mod theme;
mod types;
mod widget_config;

pub use io::{ConfigError, ThemePaths};
pub use layout::*;
pub use media::*;
pub use rules::*;
pub use theme::*;
pub use types::*;
pub use widget_config::*;
