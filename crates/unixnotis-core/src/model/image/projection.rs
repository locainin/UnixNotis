//! Lightweight notification image projections

use super::{ImageData, NotificationImage};

impl NotificationImage {
    #[must_use]
    pub fn for_listing(&self) -> Self {
        if self.image_data.data.is_empty() {
            return self.clone();
        }
        Self {
            has_image_data: false,
            image_data: ImageData::default(),
            has_conversation_avatar: self.has_conversation_avatar,
            conversation_avatar: self.conversation_avatar.clone(),
            image_path: self.image_path.clone(),
            icon_name: self.icon_name.clone(),
        }
    }

    #[must_use]
    pub fn for_history(&self) -> Self {
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
