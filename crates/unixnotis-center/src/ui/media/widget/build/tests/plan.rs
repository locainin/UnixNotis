use unixnotis_core::{
    MediaArtPosition, MediaConfig, MediaControlsPosition, MediaLayout, MediaNavigationPosition,
};

use super::super::shell::MediaShellConfig;
use super::plan::{nav_cluster_spacing_px, ShellCompositionPlan};

#[test]
fn composition_plan_matches_carousel_defaults() {
    // The default shell is the compatibility baseline for every placement flag
    let shell = MediaShellConfig::from_config(&MediaConfig::default());
    let plan = ShellCompositionPlan::from_shell(&shell);

    assert!(
        plan.start_art,
        "carousel art should begin beside the metadata"
    );
    assert!(
        plan.inline_controls,
        "carousel controls should remain inline"
    );
    assert!(
        plan.external_nav,
        "carousel player navigation should remain external"
    );
    assert!(
        !plan.top_art,
        "carousel should not move artwork above the metadata"
    );
    assert!(
        !plan.bottom_controls,
        "carousel should not create a bottom control strip"
    );
    assert!(
        !plan.side_controls,
        "carousel should not create a side action rail"
    );
}

#[test]
fn composition_plan_tracks_bottom_strip_overrides() {
    // Explicit overrides must replace showcase side routing as one coherent plan
    let config = MediaConfig {
        layout: MediaLayout::Showcase,
        controls_position: MediaControlsPosition::Bottom,
        navigation_position: MediaNavigationPosition::WithControls,
        ..MediaConfig::default()
    };
    let plan = ShellCompositionPlan::from_shell(&MediaShellConfig::from_config(&config));

    assert!(
        plan.bottom_controls,
        "bottom controls should select the lower strip"
    );
    assert!(
        plan.bottom_nav,
        "with-controls navigation should share the lower strip"
    );
    assert!(
        !plan.side_controls,
        "the control cluster should have one placement"
    );
    assert!(
        !plan.external_nav,
        "in-card navigation should remove external arrows"
    );
}

#[test]
fn composition_plan_tracks_hidden_controls_and_top_art() {
    // Independent art and control overrides must not revive hidden regions
    let config = MediaConfig {
        layout: MediaLayout::Inline,
        art_position: MediaArtPosition::Top,
        controls_position: MediaControlsPosition::Hidden,
        navigation_position: MediaNavigationPosition::WithControls,
        ..MediaConfig::default()
    };
    let plan = ShellCompositionPlan::from_shell(&MediaShellConfig::from_config(&config));

    assert!(
        plan.top_art,
        "top artwork should select the upper art region"
    );
    assert!(
        !plan.start_art,
        "artwork cannot occupy top and start regions together"
    );
    assert!(
        !plan.inline_controls,
        "hidden controls should not occupy the inline row"
    );
    assert!(
        !plan.bottom_controls,
        "hidden controls should not occupy the bottom row"
    );
    assert!(
        plan.bottom_nav,
        "navigation should remain available in its resolved strip"
    );
}

#[test]
fn composition_plan_tracks_player_preset_defaults() {
    // Player layout keeps artwork and controls in its compact vertical shell
    let config = MediaConfig {
        layout: MediaLayout::Player,
        ..MediaConfig::default()
    };
    let plan = ShellCompositionPlan::from_shell(&MediaShellConfig::from_config(&config));

    assert!(
        plan.top_art,
        "player artwork should stay above its metadata"
    );
    assert!(
        plan.bottom_controls,
        "player controls should stay in the bottom dock"
    );
    assert!(
        !plan.start_art,
        "player artwork should not use the carousel slot"
    );
    assert!(
        !plan.inline_controls,
        "player controls should not share the title row"
    );
    assert!(
        !plan.inline_nav,
        "player navigation should not consume title width"
    );
    assert!(
        !plan.bottom_nav,
        "the player preset hides player-switch navigation"
    );
    assert!(
        !plan.external_nav,
        "the player preset should remain self-contained"
    );
}

#[test]
fn composition_plan_tracks_showcase_side_rail_defaults() {
    // Showcase is the only stock layout that routes both groups into the side rail
    let config = MediaConfig {
        layout: MediaLayout::Showcase,
        ..MediaConfig::default()
    };
    let plan = ShellCompositionPlan::from_shell(&MediaShellConfig::from_config(&config));

    assert!(
        plan.side_controls,
        "showcase controls should use the side rail"
    );
    assert!(
        plan.side_nav,
        "showcase navigation should use the side rail"
    );
    assert!(
        !plan.inline_controls,
        "showcase controls should have one placement"
    );
    assert!(
        !plan.external_nav,
        "showcase navigation should stay inside the card"
    );
}

#[test]
fn compact_player_overrides_keep_the_shell_self_contained() {
    // Geometry changes should never alter structural placement decisions
    let config = MediaConfig {
        layout: MediaLayout::Player,
        art_size_px: 40,
        text_width_floor_px: 92,
        card_height_px: Some(156),
        content_spacing_px: 4,
        control_spacing_px: 4,
        ..MediaConfig::default()
    };
    let plan = ShellCompositionPlan::from_shell(&MediaShellConfig::from_config(&config));

    assert!(
        plan.top_art,
        "geometry overrides should not reroute artwork"
    );
    assert!(
        plan.bottom_controls,
        "geometry overrides should not reroute controls"
    );
    assert!(
        !plan.start_art,
        "compact geometry should preserve player composition"
    );
    assert!(
        !plan.inline_controls,
        "compact geometry should preserve the control dock"
    );
    assert!(
        !plan.external_nav,
        "compact geometry should not revive external arrows"
    );
}

#[test]
fn navigation_cluster_spacing_requires_both_neighbor_groups() {
    // The inter-group gap exists only when both groups share one strip
    let config = MediaConfig {
        navigation_spacing_px: 13,
        ..MediaConfig::default()
    };
    let shell = MediaShellConfig::from_config(&config);

    assert_eq!(nav_cluster_spacing_px(true, true, &shell), 13);
    assert_eq!(nav_cluster_spacing_px(true, false, &shell), 0);
    assert_eq!(nav_cluster_spacing_px(false, true, &shell), 0);
    assert_eq!(nav_cluster_spacing_px(false, false, &shell), 0);
}
