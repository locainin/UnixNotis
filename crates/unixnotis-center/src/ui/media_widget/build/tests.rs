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

#[test]
fn composition_plan_tracks_player_preset_defaults() {
    // The dedicated player preset should stay centered and self-contained by default
    let mut config = MediaConfig::default();
    config.layout = MediaLayout::Player;
    let plan = ShellCompositionPlan::from_shell(&MediaShellConfig::from_config(&config));

    assert!(plan.top_art);
    assert!(!plan.start_art);
    assert!(!plan.inline_controls);
    assert!(plan.bottom_controls);
    assert!(!plan.inline_nav);
    assert!(!plan.bottom_nav);
    assert!(!plan.external_nav);
}

#[test]
fn compact_player_overrides_keep_the_shell_self_contained() {
    // Smaller player cards should keep the same routing rules instead of drifting into carousel flow
    let mut config = MediaConfig::default();
    config.layout = MediaLayout::Player;
    config.art_size_px = 40;
    config.text_width_floor_px = 92;
    config.card_height_px = Some(156);
    config.content_spacing_px = 4;
    config.control_spacing_px = 4;
    let plan = ShellCompositionPlan::from_shell(&MediaShellConfig::from_config(&config));

    assert!(plan.top_art);
    assert!(!plan.start_art);
    assert!(plan.bottom_controls);
    assert!(!plan.inline_controls);
    assert!(!plan.external_nav);
}
