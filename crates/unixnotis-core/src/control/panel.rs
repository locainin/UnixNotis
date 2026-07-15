//! Panel action and debug request wire types

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use zbus::zvariant::Type;

/// Panel visibility actions sent to the UI
#[derive(Debug, Copy, Clone, Serialize_repr, Deserialize_repr, Type)]
#[repr(u32)]
pub enum PanelAction {
    Open = 0,
    Close = 1,
    Toggle = 2,
}

/// Debug verbosity for panel diagnostics requested through control tooling
#[derive(
    Debug,
    Copy,
    Clone,
    Serialize_repr,
    Deserialize_repr,
    Type,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Default,
)]
#[repr(u8)]
pub enum PanelDebugLevel {
    #[default]
    Off = 0,
    Critical = 1,
    Warn = 2,
    Info = 3,
    Verbose = 4,
}

impl PanelDebugLevel {
    #[must_use]
    pub fn allows(self, level: Self) -> bool {
        self != Self::Off && self >= level
    }
}

/// Panel request payload combining action and requested debug verbosity
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Type)]
pub struct PanelRequest {
    pub action: PanelAction,
    pub debug: PanelDebugLevel,
}

impl PanelRequest {
    #[must_use]
    pub const fn open() -> Self {
        Self {
            action: PanelAction::Open,
            debug: PanelDebugLevel::Off,
        }
    }

    #[must_use]
    pub const fn open_debug(level: PanelDebugLevel) -> Self {
        Self {
            action: PanelAction::Open,
            debug: level,
        }
    }

    #[must_use]
    pub const fn close() -> Self {
        Self {
            action: PanelAction::Close,
            debug: PanelDebugLevel::Off,
        }
    }

    #[must_use]
    pub const fn toggle() -> Self {
        Self {
            action: PanelAction::Toggle,
            debug: PanelDebugLevel::Off,
        }
    }
}

#[cfg(test)]
#[path = "tests/panel.rs"]
mod tests;
