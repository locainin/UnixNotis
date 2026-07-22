//! Command slider construction, action handling, polling, and state refresh

// User-driven command dispatch and debounce behavior
mod actions;
// Command reads, polling, watches, and widget-state application
mod refresh;
// Numeric parsing, display formatting, and change comparisons
mod value;
// GTK construction, layout, and icon resolution
mod view;
// Widget shell that connects each focused subsystem
mod widget;

pub use widget::CommandSlider;
