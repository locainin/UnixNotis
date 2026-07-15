//! CSS loading, validation, and hot-reload support shared by `UnixNotis` UIs

// Split CSS responsibilities into focused modules to keep files readable
mod loader;
mod manager;
mod overrides;
mod watch;

pub use manager::{CssKind, CssManager};
pub use watch::{start_config_watcher, start_css_watcher};

use unixnotis_core::DEFAULT_BASE_CSS;

pub const DEFAULT_CSS: &str = DEFAULT_BASE_CSS;
