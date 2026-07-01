//! Lightweight notification image projections

use super::{ImageData, NotificationImage};

impl NotificationImage {
    pub fn for_listing(&self) -> Self {
        if self.image_data.data.is_empty() {
            return self.clone();
        }
        Self {
            has_image_data: false,
            image_data: ImageData::default(),
            image_path: self.image_path.clone(),
            icon_name: self.icon_name.clone(),
        }
    }

    pub fn for_history(&self) -> NotificationImage {
        if self.has_image_data && (!self.image_path.is_empty() || !self.icon_name.is_empty()) {
            let mut trimmed = self.clone();
            // History rows can use a path or theme name, so raw bytes are dropped
            trimmed.has_image_data = false;
            trimmed.image_data = ImageData::default();
            return trimmed;
        }
        self.clone()
    }
}
