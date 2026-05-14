use super::{
    default_art_position_for_layout, default_card_height_for_layout,
    default_controls_position_for_layout, default_navigation_position_for_layout, MediaArtPosition,
    MediaConfig, MediaControlsPosition, MediaNavigationPosition,
};

impl MediaConfig {
    pub fn effective_art_position(&self) -> MediaArtPosition {
        if !self.show_art {
            // Hidden wins even when a layout preset normally shows art
            return MediaArtPosition::Hidden;
        }

        match self.art_position {
            MediaArtPosition::Auto => default_art_position_for_layout(self.layout),
            position => position,
        }
    }

    pub fn effective_controls_position(&self) -> MediaControlsPosition {
        if !self.show_controls {
            // Disabled controls should not leak back in through Auto defaults
            return MediaControlsPosition::Hidden;
        }

        match self.controls_position {
            MediaControlsPosition::Auto => default_controls_position_for_layout(self.layout),
            position => position,
        }
    }

    pub fn effective_navigation_position(&self) -> MediaNavigationPosition {
        if !self.show_navigation {
            // Navigation follows the same override rule as controls and art
            return MediaNavigationPosition::Hidden;
        }

        match self.navigation_position {
            MediaNavigationPosition::Auto => default_navigation_position_for_layout(self.layout),
            position => position,
        }
    }

    pub fn effective_card_height_px(&self) -> i32 {
        // Explicit config wins; otherwise the selected layout owns the height
        self.card_height_px
            .unwrap_or_else(|| default_card_height_for_layout(self.layout))
    }
}

#[cfg(test)]
#[path = "../tests/media.rs"]
mod tests;
