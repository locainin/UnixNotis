//! Configuration module wiring for UnixNotis.
//!
//! Keeps config types, I/O, and runtime defaults in separate files.

mod config_io;
mod config_runtime;
mod config_types;

pub use config_io::{ConfigError, ThemePaths};
pub use config_types::*;
