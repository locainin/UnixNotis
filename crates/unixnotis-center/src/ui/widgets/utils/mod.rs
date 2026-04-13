//! Shared widget helpers and command plumbing

// Command execution and queueing internals
mod command;
// Command-driven slider widget implementation
mod command_slider;
// Shared refresh backoff policy used by cards and stats
mod refresh_backoff;
// Slider icon fallback and theme compatibility helpers
mod slider_icons;
// Slider value parsing and muted-state helpers
mod slider_parse;
// Shared watch cleanup worker keeps teardown off the GTK thread
mod watch_reaper;
// Long-running command watch lifecycle helpers
mod watch;

// Shared command helpers are scoped to widget internals
pub(super) use command::{
    run_command, run_command_capture_action_async, run_command_capture_async,
    run_command_capture_status_async, run_command_capture_with_timeout_async,
};
// Public re-export keeps widget wrappers concise
pub use command_slider::CommandSlider;
// Backoff policy is reused by polling widgets
pub(super) use refresh_backoff::RefreshBackoff;
// Watcher helpers are reused by sliders and toggles
pub(super) use watch::{start_command_watch, CommandWatch};
