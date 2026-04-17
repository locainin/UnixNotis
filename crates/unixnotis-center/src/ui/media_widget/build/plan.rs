use super::super::shell::{
    MediaShellConfig, ResolvedMediaArtPosition, ResolvedMediaControlsPosition,
    ResolvedMediaNavigationPosition,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ShellCompositionPlan {
    // These flags describe which shared shell regions are active for one config snapshot
    pub(super) top_art: bool,
    pub(super) start_art: bool,
    pub(super) inline_controls: bool,
    pub(super) inline_nav: bool,
    pub(super) bottom_controls: bool,
    pub(super) bottom_nav: bool,
    pub(super) side_controls: bool,
    pub(super) side_nav: bool,
    pub(super) external_nav: bool,
}

impl ShellCompositionPlan {
    pub(super) fn from_shell(shell: &MediaShellConfig) -> Self {
        // This keeps the later build code on plain booleans instead of repeated enum checks
        Self {
            top_art: shell.art_position == ResolvedMediaArtPosition::Top,
            start_art: shell.art_position == ResolvedMediaArtPosition::Start,
            inline_controls: shell.controls_position == ResolvedMediaControlsPosition::Inline,
            inline_nav: shell.navigation_position == ResolvedMediaNavigationPosition::Inline,
            bottom_controls: shell.controls_position == ResolvedMediaControlsPosition::Bottom,
            bottom_nav: shell.navigation_position == ResolvedMediaNavigationPosition::Bottom,
            side_controls: shell.controls_position == ResolvedMediaControlsPosition::Side,
            side_nav: shell.navigation_position == ResolvedMediaNavigationPosition::Side,
            external_nav: shell.navigation_position == ResolvedMediaNavigationPosition::External,
        }
    }
}

pub(super) fn nav_cluster_spacing_px(
    include_controls: bool,
    include_nav: bool,
    shell: &MediaShellConfig,
) -> i32 {
    // Nav spacing owns the gap between transport controls and player switching controls
    if include_controls && include_nav {
        return shell.navigation_spacing_px;
    }
    // A single group does not need extra gap padding around itself
    0
}
