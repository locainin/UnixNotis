//! Timed Do Not Disturb menu configuration

use serde::{Deserialize, Serialize};

/// Input gestures that can open the timed DND menu
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum DndMenuTrigger {
    RightClick,
    LongPress,
    Keyboard,
}

/// One typed deadline shown in the timed DND menu
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "kebab-case")]
pub enum DndMenuChoice {
    /// Enable DND for a relative number of minutes
    Duration { label: String, minutes: u32 },
    /// Enable DND until a clock time on the next local calendar day
    Tomorrow { label: String, hour: u8, minute: u8 },
    /// Enable DND without an expiration deadline
    Indefinite { label: String },
}

impl DndMenuChoice {
    /// Return the user-facing menu label
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Duration { label, .. }
            | Self::Tomorrow { label, .. }
            | Self::Indefinite { label } => label,
        }
    }

    /// Return mutable access to the user-facing menu label
    pub(in crate::config) const fn label_mut(&mut self) -> &mut String {
        match self {
            Self::Duration { label, .. }
            | Self::Tomorrow { label, .. }
            | Self::Indefinite { label } => label,
        }
    }
}

/// Return the stock DND menu input policy
#[must_use]
pub fn default_dnd_menu_triggers() -> Vec<DndMenuTrigger> {
    // Secondary click is the only default path so ordinary pointer use stays quiet
    vec![DndMenuTrigger::RightClick]
}

/// Return the stock DND deadline menu
#[must_use]
pub fn default_dnd_menu_choices() -> Vec<DndMenuChoice> {
    vec![
        DndMenuChoice::Duration {
            label: "30 minutes".to_string(),
            minutes: 30,
        },
        DndMenuChoice::Duration {
            label: "1 hour".to_string(),
            minutes: 60,
        },
        DndMenuChoice::Duration {
            label: "2 hours".to_string(),
            minutes: 120,
        },
        DndMenuChoice::Tomorrow {
            label: "Until tomorrow morning".to_string(),
            hour: 8,
            minute: 0,
        },
        DndMenuChoice::Indefinite {
            label: "Indefinitely".to_string(),
        },
    ]
}
