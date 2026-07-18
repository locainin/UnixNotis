use unixnotis_core::{
    MediaArtPosition, MediaConfig, MediaControlsPosition, MediaLayout, MediaNavigationPosition,
};

use super::{
    MediaShellConfig, ResolvedMediaArtPosition, ResolvedMediaControlsPosition,
    ResolvedMediaNavigationPosition,
};

#[test]
fn shell_defaults_follow_layout_preset() {
    let config = MediaConfig::default();
    let shell = MediaShellConfig::from_config(&config);

    assert_eq!(shell.layout, MediaLayout::Carousel);
    assert_eq!(shell.art_position, ResolvedMediaArtPosition::Start);
    assert_eq!(
        shell.controls_position,
        ResolvedMediaControlsPosition::Inline
    );
    assert_eq!(
        shell.navigation_position,
        ResolvedMediaNavigationPosition::External
    );
}

#[test]
fn shell_overrides_can_mix_positions() {
    let config = MediaConfig {
        layout: MediaLayout::Showcase,
        art_position: MediaArtPosition::Top,
        controls_position: MediaControlsPosition::Bottom,
        navigation_position: MediaNavigationPosition::WithControls,
        ..MediaConfig::default()
    };
    let shell = MediaShellConfig::from_config(&config);

    assert_eq!(shell.art_position, ResolvedMediaArtPosition::Top);
    assert_eq!(
        shell.controls_position,
        ResolvedMediaControlsPosition::Bottom
    );
    assert_eq!(
        shell.navigation_position,
        ResolvedMediaNavigationPosition::Bottom
    );
}

#[test]
fn hidden_controls_keep_navigation_inside_card() {
    let config = MediaConfig {
        controls_position: MediaControlsPosition::Hidden,
        navigation_position: MediaNavigationPosition::WithControls,
        ..MediaConfig::default()
    };
    let shell = MediaShellConfig::from_config(&config);

    assert_eq!(
        shell.controls_position,
        ResolvedMediaControlsPosition::Hidden
    );
    assert_eq!(
        shell.navigation_position,
        ResolvedMediaNavigationPosition::Bottom
    );
}

#[test]
fn player_layout_resolves_to_top_art_and_no_nav() {
    let config = MediaConfig {
        layout: MediaLayout::Player,
        ..MediaConfig::default()
    };
    let shell = MediaShellConfig::from_config(&config);

    assert_eq!(shell.art_position, ResolvedMediaArtPosition::Top);
    assert_eq!(
        shell.controls_position,
        ResolvedMediaControlsPosition::Bottom
    );
    assert_eq!(
        shell.navigation_position,
        ResolvedMediaNavigationPosition::Hidden
    );
}
