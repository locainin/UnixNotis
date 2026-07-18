//! Command slider construction, action handling, polling, and state refresh

// User-driven command dispatch and debounce behavior
mod actions;
// Command reads, polling, watches, and widget-state application
mod refresh;
// Numeric parsing, display formatting, and change comparisons
mod value;
// GTK construction, layout, and icon resolution
mod view;
// Public widget shell that connects each focused subsystem
mod widget;

use super::{
    run_action_command_with_completion, run_command_capture_status_async, start_command_watch,
    CommandWatch, RefreshBackoff, INFLIGHT_REFRESH_RECHECK,
};
pub use widget::CommandSlider;
