use gtk::prelude::*;
use unixnotis_core::{
    hooks, MediaArtPosition, MediaConfig, MediaControlsPosition, MediaLayout,
    MediaNavigationPosition,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ResolvedMediaArtPosition {
    Start,
    Top,
    Hidden,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ResolvedMediaControlsPosition {
    Inline,
    Bottom,
    Side,
    Hidden,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ResolvedMediaNavigationPosition {
    External,
    Inline,
    Bottom,
    Side,
    Hidden,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MediaShellConfig {
    // Layout stays here so widget builders and width math use the same resolved shell snapshot
    pub(super) layout: MediaLayout,
    pub(super) art_position: ResolvedMediaArtPosition,
    pub(super) controls_position: ResolvedMediaControlsPosition,
    pub(super) navigation_position: ResolvedMediaNavigationPosition,
    pub(super) art_size_px: i32,
    pub(super) text_width_floor_px: i32,
    pub(super) card_height_px: i32,
    pub(super) content_spacing_px: i32,
    pub(super) control_spacing_px: i32,
    pub(super) navigation_spacing_px: i32,
}

impl MediaShellConfig {
    pub(super) fn from_config(config: &MediaConfig) -> Self {
        // Resolve every auto mode once so later code does not re-run layout defaults differently
        let controls_position = match config.effective_controls_position() {
            MediaControlsPosition::Inline => ResolvedMediaControlsPosition::Inline,
            MediaControlsPosition::Bottom => ResolvedMediaControlsPosition::Bottom,
            MediaControlsPosition::Side => ResolvedMediaControlsPosition::Side,
            MediaControlsPosition::Hidden | MediaControlsPosition::Auto => {
                ResolvedMediaControlsPosition::Hidden
            }
        };
        let navigation_position = resolve_navigation_position(config, controls_position);

        Self {
            layout: config.layout,
            art_position: match config.effective_art_position() {
                MediaArtPosition::Start => ResolvedMediaArtPosition::Start,
                MediaArtPosition::Top => ResolvedMediaArtPosition::Top,
                MediaArtPosition::Hidden | MediaArtPosition::Auto => {
                    ResolvedMediaArtPosition::Hidden
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
}

fn resolve_navigation_position(
    config: &MediaConfig,
    controls_position: ResolvedMediaControlsPosition,
) -> ResolvedMediaNavigationPosition {
    // Navigation follows the resolved control rail so mixed shells stay internally consistent
    match config.effective_navigation_position() {
        MediaNavigationPosition::External => ResolvedMediaNavigationPosition::External,
        MediaNavigationPosition::WithControls => match controls_position {
            ResolvedMediaControlsPosition::Inline => ResolvedMediaNavigationPosition::Inline,
            ResolvedMediaControlsPosition::Bottom => ResolvedMediaNavigationPosition::Bottom,
            ResolvedMediaControlsPosition::Side => ResolvedMediaNavigationPosition::Side,
            // When controls are hidden, keep navigation inside the card rather than forcing it out
            ResolvedMediaControlsPosition::Hidden => ResolvedMediaNavigationPosition::Bottom,
        },
        MediaNavigationPosition::Hidden | MediaNavigationPosition::Auto => {
            ResolvedMediaNavigationPosition::Hidden
        }
    }
}

pub(super) fn apply_shell_state_classes<W: IsA<gtk::Widget>>(widget: &W, shell: &MediaShellConfig) {
    let widget = widget.as_ref();
    // State classes are applied to every structural box so CSS can style any shell depth directly
    set_class_state(
        widget,
        hooks::media_shell::HAS_CONTROLS,
        shell.controls_position != ResolvedMediaControlsPosition::Hidden,
    );
    set_class_state(
        widget,
        hooks::media_shell::NO_CONTROLS,
        shell.controls_position == ResolvedMediaControlsPosition::Hidden,
    );
    set_class_state(
        widget,
        hooks::media_shell::HAS_NAV,
        shell.navigation_position != ResolvedMediaNavigationPosition::Hidden,
    );
    set_class_state(
        widget,
        hooks::media_shell::NO_NAV,
        shell.navigation_position == ResolvedMediaNavigationPosition::Hidden,
    );

    set_class_state(
        widget,
        hooks::media_shell::ART_START,
        shell.art_position == ResolvedMediaArtPosition::Start,
    );
    set_class_state(
        widget,
        hooks::media_shell::ART_TOP,
        shell.art_position == ResolvedMediaArtPosition::Top,
    );
    set_class_state(
        widget,
        hooks::media_shell::ART_HIDDEN,
        shell.art_position == ResolvedMediaArtPosition::Hidden,
    );

    set_class_state(
        widget,
        hooks::media_shell::CONTROLS_INLINE,
        shell.controls_position == ResolvedMediaControlsPosition::Inline,
    );
    set_class_state(
        widget,
        hooks::media_shell::CONTROLS_BOTTOM,
        shell.controls_position == ResolvedMediaControlsPosition::Bottom,
    );
    set_class_state(
        widget,
        hooks::media_shell::CONTROLS_SIDE,
        shell.controls_position == ResolvedMediaControlsPosition::Side,
    );
    set_class_state(
        widget,
        hooks::media_shell::CONTROLS_HIDDEN,
        shell.controls_position == ResolvedMediaControlsPosition::Hidden,
    );

    set_class_state(
        widget,
        hooks::media_shell::NAV_EXTERNAL,
        shell.navigation_position == ResolvedMediaNavigationPosition::External,
    );
    set_class_state(
        widget,
        hooks::media_shell::NAV_INLINE,
        shell.navigation_position == ResolvedMediaNavigationPosition::Inline,
    );
    set_class_state(
        widget,
        hooks::media_shell::NAV_BOTTOM,
        shell.navigation_position == ResolvedMediaNavigationPosition::Bottom,
    );
    set_class_state(
        widget,
        hooks::media_shell::NAV_SIDE,
        shell.navigation_position == ResolvedMediaNavigationPosition::Side,
    );
    set_class_state(
        widget,
        hooks::media_shell::NAV_HIDDEN,
        shell.navigation_position == ResolvedMediaNavigationPosition::Hidden,
    );
}

fn set_class_state(widget: &gtk::Widget, class_name: &str, enabled: bool) {
    if enabled {
        if !widget.has_css_class(class_name) {
            widget.add_css_class(class_name);
        }
    } else if widget.has_css_class(class_name) {
        widget.remove_css_class(class_name);
    }
}

#[cfg(test)]
#[path = "tests/shell.rs"]
mod tests;
