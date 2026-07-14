//! Core notification enum and action types shared across models

use serde::{Deserialize, Serialize};
use zbus::zvariant::{OwnedValue, Type};

/// Notification urgency levels defined by the specification
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[repr(u8)]
pub enum Urgency {
    Low = 0,
    Normal = 1,
    Critical = 2,
}

impl Urgency {
    #[must_use]
    pub fn from_hint(value: Option<&OwnedValue>) -> Self {
        let Some(value) = value else {
            return Self::Normal;
        };
        // Some clients send byte hints and others send wider integers
        let level = if let Ok(v) = u8::try_from(value) {
            u32::from(v)
        } else if let Ok(v) = u32::try_from(value) {
            v
        } else {
            return Self::Normal;
        };

        match level {
            0 => Self::Low,
            2 => Self::Critical,
            // Unknown values fall back to normal per freedesktop-friendly behavior
            _ => Self::Normal,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Action pair in the notification protocol
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct Action {
    pub key: String,
    pub label: String,
}

#[cfg(test)]
#[path = "tests/types.rs"]
mod tests;
