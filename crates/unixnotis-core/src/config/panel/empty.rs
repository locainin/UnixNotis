use serde::{Deserialize, Serialize};

/// Placement of the empty notification message inside the remaining list area
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum EmptyStateAlignment {
    /// Follow the legacy behavior: start below widgets and center when widgets are hidden
    #[default]
    Auto,
    /// Keep the message at the start of the remaining notification area
    Start,
    /// Center the message inside the remaining notification area
    Center,
    /// Keep the message at the end of the remaining notification area
    End,
}
