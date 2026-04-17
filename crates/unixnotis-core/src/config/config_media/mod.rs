//! Media widget configuration and shared layout defaults
//!
//! Keeps the media surface in one place so the center, tools, and tests all
//! reason about the same runtime contract

mod defaults;
#[cfg(test)]
mod tests;
mod types;

pub use self::defaults::{
    default_art_position_for_layout, default_card_height_for_layout,
    default_controls_position_for_layout, default_navigation_position_for_layout,
    DEFAULT_MEDIA_ART_SIZE_PX, DEFAULT_MEDIA_TEXT_WIDTH_FLOOR_PX,
};
pub use self::types::{
    MediaArtPosition, MediaConfig, MediaControlsPosition, MediaLayout, MediaNavigationPosition,
    MediaPositionFormat, MediaRemoteArtPolicy, MediaTitleFallback,
};

impl MediaConfig {
    pub fn effective_art_position(&self) -> MediaArtPosition {
        if !self.show_art {
            return MediaArtPosition::Hidden;
        }
        match self.art_position {
            MediaArtPosition::Auto => default_art_position_for_layout(self.layout),
            position => position,
        }
    }

    pub fn effective_controls_position(&self) -> MediaControlsPosition {
        if !self.show_controls {
            return MediaControlsPosition::Hidden;
        }
        match self.controls_position {
            MediaControlsPosition::Auto => default_controls_position_for_layout(self.layout),
            position => position,
        }
    }

    pub fn effective_navigation_position(&self) -> MediaNavigationPosition {
        if !self.show_navigation {
            return MediaNavigationPosition::Hidden;
        }
        match self.navigation_position {
            MediaNavigationPosition::Auto => default_navigation_position_for_layout(self.layout),
            position => position,
        }
    }

    pub fn effective_card_height_px(&self) -> i32 {
        self.card_height_px
            .unwrap_or_else(|| default_card_height_for_layout(self.layout))
    }
}
