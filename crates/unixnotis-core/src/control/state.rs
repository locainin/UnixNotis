//! Control-plane state shared across daemon and UI processes

use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

/// Control-plane state broadcast to the UI
#[derive(Debug, Clone, Serialize, Deserialize, Type, Default, PartialEq, Eq)]
pub struct ControlState {
    pub dnd_enabled: bool,
    /// Unix timestamp in seconds, or zero for an indefinite/disabled state
    pub dnd_expires_at: i64,
    pub history_count: u32,
    /// True when at least one active inhibitor suppresses popups
    pub inhibited: bool,
    /// Total number of active inhibitors across all scopes
    pub inhibitor_count: u32,
}

/// Popup gating fields that affect toast visibility
#[derive(Debug, Clone, Serialize, Deserialize, Type, Default, PartialEq, Eq)]
pub struct PopupGateState {
    pub dnd_enabled: bool,
    pub inhibited: bool,
}

/// Process and handshake state for both daemon-managed user interfaces
#[derive(Debug, Clone, Serialize, Deserialize, Type, Default, PartialEq, Eq)]
pub struct UiHealth {
    pub center_process_running: bool,
    pub center_ready: bool,
    pub popups_process_running: bool,
    pub popups_ready: bool,
}

/// Tuple layout for inhibitor listings: identifier, reason, scope, and owner
pub type InhibitorInfo = (u64, String, u32, String);
