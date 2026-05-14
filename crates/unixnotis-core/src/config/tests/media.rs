use crate::{
    MediaArtPosition, MediaConfig, MediaControlsPosition, MediaLayout, MediaNavigationPosition,
};

#[test]
fn preset_defaults_stay_stable() {
    let mut config = MediaConfig {
        layout: MediaLayout::Carousel,
        ..MediaConfig::default()
    };
    assert_eq!(config.effective_art_position(), MediaArtPosition::Start);
    assert_eq!(
        config.effective_controls_position(),
        MediaControlsPosition::Inline
    );
    assert_eq!(
        config.effective_navigation_position(),
        MediaNavigationPosition::External
    );

    config.layout = MediaLayout::Showcase;
    assert_eq!(config.effective_art_position(), MediaArtPosition::Start);
    assert_eq!(
        config.effective_controls_position(),
        MediaControlsPosition::Side
    );
    assert_eq!(
        config.effective_navigation_position(),
        MediaNavigationPosition::WithControls
    );
}

#[test]
fn explicit_media_positions_win_over_layout_defaults() {
    let config = MediaConfig {
        layout: MediaLayout::Showcase,
        art_position: MediaArtPosition::Top,
        controls_position: MediaControlsPosition::Bottom,
        navigation_position: MediaNavigationPosition::External,
        ..MediaConfig::default()
    };

    assert_eq!(config.effective_art_position(), MediaArtPosition::Top);
    assert_eq!(
        config.effective_controls_position(),
        MediaControlsPosition::Bottom
    );
    assert_eq!(
        config.effective_navigation_position(),
        MediaNavigationPosition::External
    );
}

#[test]
fn hidden_feature_flags_override_layout_defaults() {
    let config = MediaConfig {
        layout: MediaLayout::Player,
        show_art: false,
        show_controls: false,
        show_navigation: false,
        ..MediaConfig::default()
    };

    assert_eq!(config.effective_art_position(), MediaArtPosition::Hidden);
    assert_eq!(
        config.effective_controls_position(),
        MediaControlsPosition::Hidden
    );
    assert_eq!(
        config.effective_navigation_position(),
        MediaNavigationPosition::Hidden
    );
}

#[test]
fn preset_card_heights_follow_the_selected_layout() {
    let carousel = MediaConfig {
        layout: MediaLayout::Carousel,
        ..MediaConfig::default()
    };
    let showcase = MediaConfig {
        layout: MediaLayout::Showcase,
        ..MediaConfig::default()
    };

    assert_ne!(
        carousel.effective_card_height_px(),
        showcase.effective_card_height_px()
    );
    assert_eq!(carousel.effective_card_height_px(), 72);
    assert_eq!(showcase.effective_card_height_px(), 96);
}

#[test]
fn explicit_card_height_override_wins_over_preset_default() {
    let config = MediaConfig {
        layout: MediaLayout::Player,
        card_height_px: Some(164),
        ..MediaConfig::default()
    };

    assert_eq!(config.effective_card_height_px(), 164);
}
