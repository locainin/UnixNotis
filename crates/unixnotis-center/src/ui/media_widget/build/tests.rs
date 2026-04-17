use unixnotis_core::{
    MediaArtPosition, MediaConfig, MediaControlsPosition, MediaLayout, MediaNavigationPosition,
};

use super::super::shell::MediaShellConfig;
use super::plan::ShellCompositionPlan;

#[test]
fn composition_plan_matches_carousel_defaults() {
    // The stock carousel shell is the baseline every override test compares against
    let shell = MediaShellConfig::from_config(&MediaConfig::default());
    let plan = ShellCompositionPlan::from_shell(&shell);

    assert!(plan.start_art);
    assert!(plan.inline_controls);
    assert!(plan.external_nav);
    assert!(!plan.top_art);
    assert!(!plan.bottom_controls);
    assert!(!plan.side_controls);
}

#[test]
fn composition_plan_tracks_bottom_strip_overrides() {
    // Showcase defaults get replaced here so the lower strip routing can be checked directly
    let mut config = MediaConfig::default();
    config.layout = MediaLayout::Showcase;
    config.controls_position = MediaControlsPosition::Bottom;
    config.navigation_position = MediaNavigationPosition::WithControls;
    let plan = ShellCompositionPlan::from_shell(&MediaShellConfig::from_config(&config));

    assert!(plan.bottom_controls);
    assert!(plan.bottom_nav);
    assert!(!plan.side_controls);
    assert!(!plan.external_nav);
}

#[test]
fn composition_plan_tracks_hidden_controls_and_top_art() {
    // Hidden controls should still keep in-card nav instead of reviving the external shell
    let mut config = MediaConfig::default();
    config.layout = MediaLayout::Inline;
    config.art_position = MediaArtPosition::Top;
    config.controls_position = MediaControlsPosition::Hidden;
    config.navigation_position = MediaNavigationPosition::WithControls;
    let plan = ShellCompositionPlan::from_shell(&MediaShellConfig::from_config(&config));

    assert!(plan.top_art);
    assert!(!plan.start_art);
    assert!(!plan.inline_controls);
    assert!(!plan.bottom_controls);
    assert!(plan.bottom_nav);
}
