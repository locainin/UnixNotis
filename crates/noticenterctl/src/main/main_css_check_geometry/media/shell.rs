use unixnotis_core::{
    MediaArtPosition, MediaConfig, MediaControlsPosition, MediaNavigationPosition,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ModeledMediaArtPosition {
    Start,
    Top,
    Hidden,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ModeledMediaControlsPosition {
    Inline,
    Bottom,
    Side,
    Hidden,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ModeledMediaNavigationPosition {
    External,
    Inline,
    Bottom,
    Side,
    Hidden,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ModeledMediaShell {
    pub(super) art_position: ModeledMediaArtPosition,
    pub(super) controls_position: ModeledMediaControlsPosition,
    pub(super) navigation_position: ModeledMediaNavigationPosition,
    pub(super) art_size_px: i32,
    pub(super) text_width_floor_px: i32,
    pub(super) card_height_px: i32,
    pub(super) content_spacing_px: i32,
    pub(super) control_spacing_px: i32,
    pub(super) navigation_spacing_px: i32,
}

impl ModeledMediaShell {
    pub(super) fn from_config(config: &MediaConfig) -> Self {
        // Control placement is resolved first because nav placement can follow it
        let controls_position = match config.effective_controls_position() {
            MediaControlsPosition::Inline => ModeledMediaControlsPosition::Inline,
            MediaControlsPosition::Bottom => ModeledMediaControlsPosition::Bottom,
            MediaControlsPosition::Side => ModeledMediaControlsPosition::Side,
            MediaControlsPosition::Hidden | MediaControlsPosition::Auto => {
                ModeledMediaControlsPosition::Hidden
            }
        };

        let navigation_position = match config.effective_navigation_position() {
            MediaNavigationPosition::External => ModeledMediaNavigationPosition::External,
            MediaNavigationPosition::WithControls => match controls_position {
                ModeledMediaControlsPosition::Inline => ModeledMediaNavigationPosition::Inline,
                ModeledMediaControlsPosition::Bottom => ModeledMediaNavigationPosition::Bottom,
                ModeledMediaControlsPosition::Side => ModeledMediaNavigationPosition::Side,
                // Hidden controls still keep nav in the card instead of reviving the old carousel shell
                ModeledMediaControlsPosition::Hidden => ModeledMediaNavigationPosition::Bottom,
            },
            MediaNavigationPosition::Hidden | MediaNavigationPosition::Auto => {
                ModeledMediaNavigationPosition::Hidden
            }
        };

        Self {
            // Art placement follows the resolved runtime config instead of raw user input
            art_position: match config.effective_art_position() {
                MediaArtPosition::Start => ModeledMediaArtPosition::Start,
                MediaArtPosition::Top => ModeledMediaArtPosition::Top,
                MediaArtPosition::Hidden | MediaArtPosition::Auto => {
                    ModeledMediaArtPosition::Hidden
                }
            },
            controls_position,
            navigation_position,
            art_size_px: config.art_size_px,
            text_width_floor_px: config.text_width_floor_px,
            card_height_px: config.effective_card_height_px(),
            content_spacing_px: config.content_spacing_px,
            control_spacing_px: config.control_spacing_px,
            navigation_spacing_px: config.navigation_spacing_px,
        }
    }

    pub(super) fn art_frame_size_px(self) -> i32 {
        // The runtime shell adds a small frame around the configured art size
        self.art_size_px.saturating_add(4)
    }

    pub(super) fn has_vertical_height_risk(self) -> bool {
        // Single-band shells stay stable enough that height lint would only add noise
        self.art_position == ModeledMediaArtPosition::Top
            || self.controls_position == ModeledMediaControlsPosition::Bottom
            || self.controls_position == ModeledMediaControlsPosition::Side
            || self.navigation_position == ModeledMediaNavigationPosition::Bottom
            || self.navigation_position == ModeledMediaNavigationPosition::Side
    }
}

pub(super) fn nav_cluster_spacing_px(
    include_controls: bool,
    include_nav: bool,
    shell: ModeledMediaShell,
) -> i32 {
    // Match the GTK shell: nav spacing owns the gap between transport and player switching controls
    if include_controls && include_nav {
        return shell.navigation_spacing_px;
    }
    // One active group does not spend the shared inter-cluster gap
    0
}
