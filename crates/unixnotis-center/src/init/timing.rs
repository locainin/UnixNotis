//! Shared startup interaction timing

// Short guard for buttons that send daemon commands
// Prevents double-click bursts from queueing duplicate actions
pub(super) const CONTROL_CLICK_GUARD_MS: u64 = 180;

// Tiny coalescing window for the widget collapse event
// Keeps rapid toggle edges from flooding the main event queue
pub(super) const WIDGETS_TOGGLE_COALESCE_MS: u64 = 16;
