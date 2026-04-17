use super::{
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
fn explicit_card_height_override_wins_over_preset_default() {
    let config = MediaConfig {
        layout: MediaLayout::Player,
        card_height_px: Some(164),
        ..MediaConfig::default()
    };

    assert_eq!(config.effective_card_height_px(), 164);
}
