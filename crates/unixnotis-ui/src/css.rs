//! CSS loading, validation, and hot-reload support shared by UnixNotis UIs.

// Split CSS responsibilities into focused modules to keep files readable.
mod css_loader;
mod css_manager;
mod css_overrides;
mod css_watch;

pub use css_manager::{CssKind, CssManager};
pub use css_watch::{start_config_watcher, start_css_watcher};

use unixnotis_core::DEFAULT_BASE_CSS;

pub const DEFAULT_CSS: &str = DEFAULT_BASE_CSS;
