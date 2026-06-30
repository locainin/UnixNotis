//! CSS loading, validation, and hot-reload support shared by UnixNotis UIs.

// Split CSS responsibilities into focused modules to keep files readable
#[path = "css/loader/root.rs"]
mod loader;
#[path = "css/manager/root.rs"]
mod manager;
#[path = "css/overrides.rs"]
mod overrides;
#[path = "css/watch.rs"]
mod watch;

pub use manager::{CssKind, CssManager};
pub use watch::{start_config_watcher, start_css_watcher};

use unixnotis_core::DEFAULT_BASE_CSS;

pub const DEFAULT_CSS: &str = DEFAULT_BASE_CSS;
