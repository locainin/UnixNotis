use unixnotis_core::{
    MediaArtPosition, MediaConfig, MediaControlsPosition, MediaLayout, MediaNavigationPosition,
};

use super::super::shell::MediaShellConfig;
use super::{card_height_for_shell, marquee_width_for_shell, media_content_width};

fn shell_for(layout: MediaLayout) -> MediaShellConfig {
    let mut config = MediaConfig::default();
    config.layout = layout;
    MediaShellConfig::from_config(&config)
}

#[test]
fn media_layout_reserve_budgets_stay_ordered() {
    let panel_width = 420;
    let carousel = marquee_width_for_shell(&shell_for(MediaLayout::Carousel), panel_width);
    let inline = marquee_width_for_shell(&shell_for(MediaLayout::Inline), panel_width);
    let stacked = marquee_width_for_shell(&shell_for(MediaLayout::Stacked), panel_width);
    let showcase = marquee_width_for_shell(&shell_for(MediaLayout::Showcase), panel_width);

    // Carousel spends width on both inline controls and outer navigation
    assert!(carousel < showcase);
    // Showcase still keeps a side rail, so stacked keeps the most text room
    assert!(showcase < inline);
    assert!(inline < stacked);
}

#[test]
fn media_layout_height_presets_match_expected_profiles() {
    assert_eq!(card_height_for_shell(&shell_for(MediaLayout::Carousel)), 72);
    assert_eq!(card_height_for_shell(&shell_for(MediaLayout::Inline)), 92);
    assert_eq!(card_height_for_shell(&shell_for(MediaLayout::Stacked)), 112);
    assert_eq!(card_height_for_shell(&shell_for(MediaLayout::Showcase)), 96);
}

#[test]
fn marquee_width_never_drops_below_text_floor() {
    for layout in [
        MediaLayout::Carousel,
        MediaLayout::Inline,
        MediaLayout::Stacked,
        MediaLayout::Showcase,
    ] {
        let shell = shell_for(layout);
        assert_eq!(
            marquee_width_for_shell(&shell, 80),
            shell.text_width_floor_px
        );
    }
}

#[test]
fn media_content_width_reserves_panel_surface_chrome() {
    assert_eq!(media_content_width(420), 384);
    assert_eq!(media_content_width(20), 1);
}

#[test]
fn hidden_art_and_bottom_controls_free_title_width() {
    let mut config = MediaConfig::default();
    config.layout = MediaLayout::Showcase;
    config.art_position = MediaArtPosition::Hidden;
    config.controls_position = MediaControlsPosition::Bottom;
    let shell = MediaShellConfig::from_config(&config);

    assert_eq!(marquee_width_for_shell(&shell, 420), 384);
}

#[test]
fn larger_art_slot_reduces_marquee_budget() {
    let mut config = MediaConfig::default();
    config.layout = MediaLayout::Inline;
    config.art_size_px = 88;
    let shell = MediaShellConfig::from_config(&config);

    assert_eq!(marquee_width_for_shell(&shell, 420), 282);
}

#[test]
fn larger_navigation_gap_reduces_inline_title_budget() {
    let mut config = MediaConfig::default();
    config.layout = MediaLayout::Inline;
    config.controls_position = MediaControlsPosition::Inline;
    config.navigation_position = MediaNavigationPosition::WithControls;
    config.navigation_spacing_px = 6;
    let tight_gap = MediaShellConfig::from_config(&config);

    config.navigation_spacing_px = 30;
    let wide_gap = MediaShellConfig::from_config(&config);

    assert!(marquee_width_for_shell(&wide_gap, 520) < marquee_width_for_shell(&tight_gap, 520));
}
