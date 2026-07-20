//! Angled corner geometry shared by notification surfaces

use serde::{Deserialize, Serialize};

/// Pixel cuts applied to the four corners of a rendered plate
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct CutCorners {
    /// Diagonal cut measured from the top-left corner
    pub top_left: u16,
    /// Diagonal cut measured from the top-right corner
    pub top_right: u16,
    /// Diagonal cut measured from the bottom-right corner
    pub bottom_right: u16,
    /// Diagonal cut measured from the bottom-left corner
    pub bottom_left: u16,
}

impl CutCorners {
    /// Return true when at least one corner needs path clipping
    #[must_use]
    pub const fn is_active(self) -> bool {
        self.top_left != 0 || self.top_right != 0 || self.bottom_right != 0 || self.bottom_left != 0
    }
}
