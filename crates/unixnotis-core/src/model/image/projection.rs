//! Lightweight notification image projections

use super::NotificationImage;

impl NotificationImage {
    #[must_use]
    pub fn for_listing(&self) -> Self {
        // All retained images are already bounded daemon-owned pixels
        self.clone()
    }

    #[must_use]
    pub fn for_history(&self) -> Self {
        // History receives pixels only; sender paths never cross this boundary
        self.clone()
    }
}
