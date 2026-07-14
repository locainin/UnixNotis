use super::types::{
    MediaArtPosition, MediaConfig, MediaControlsPosition, MediaLayout, MediaNavigationPosition,
    MediaPositionFormat, MediaRemoteArtPolicy, MediaTitleFallback,
};

// Compact artwork leaves more horizontal space for title metadata and controls
pub const DEFAULT_MEDIA_ART_SIZE_PX: i32 = 48;
// The floor protects short titles without forcing the panel wider
pub const DEFAULT_MEDIA_TEXT_WIDTH_FLOOR_PX: i32 = 100;

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            layout: MediaLayout::Carousel,
            // Browser MPRIS is useful by default, but remote artwork is still gated below
            include_browsers: true,
            browser_tokens: default_browser_tokens(),
            title_char_limit: 32,
            show_source: true,
            show_source_when_single_player: true,
            show_position: true,
            show_position_when_single_player: false,
            show_title: true,
            show_artist: true,
            collapse_missing_artist: false,
            show_art: true,
            collapse_missing_art: false,
            show_controls: true,
            show_navigation: true,
            title_fallback: MediaTitleFallback::Identity,
            position_format: MediaPositionFormat::Fraction,
            source_aliases: Default::default(),
            art_position: MediaArtPosition::Auto,
            controls_position: MediaControlsPosition::Auto,
            navigation_position: MediaNavigationPosition::Auto,
            art_size_px: DEFAULT_MEDIA_ART_SIZE_PX,
            text_width_floor_px: DEFAULT_MEDIA_TEXT_WIDTH_FLOOR_PX,
            card_height_px: None,
            // Tight gaps preserve clear grouping inside the stock 420px panel
            content_spacing_px: 8,
            control_spacing_px: 4,
            navigation_spacing_px: 4,
            allowlist: Vec::new(),
            denylist: vec!["playerctld".to_string()],
            // Browsers stay opt-in because webpage metadata can choose artwork URLs
            remote_art_policy: MediaRemoteArtPolicy::NativeOnly,
        }
    }
}

#[must_use]
pub const fn default_art_position_for_layout(layout: MediaLayout) -> MediaArtPosition {
    // Presets own their natural shape; explicit config can override this later
    match layout {
        MediaLayout::Stacked | MediaLayout::Player => MediaArtPosition::Top,
        MediaLayout::Carousel | MediaLayout::Inline | MediaLayout::Showcase => {
            MediaArtPosition::Start
        }
    }
}

#[must_use]
pub const fn default_controls_position_for_layout(layout: MediaLayout) -> MediaControlsPosition {
    // Control placement follows the shell shape so each preset remains balanced
    match layout {
        MediaLayout::Carousel => MediaControlsPosition::Inline,
        MediaLayout::Inline | MediaLayout::Stacked | MediaLayout::Player => {
            MediaControlsPosition::Bottom
        }
        MediaLayout::Showcase => MediaControlsPosition::Side,
    }
}

#[must_use]
pub const fn default_navigation_position_for_layout(
    layout: MediaLayout,
) -> MediaNavigationPosition {
    // Player hides navigation by default because it is designed as a focused single-card shell
    match layout {
        MediaLayout::Carousel => MediaNavigationPosition::External,
        MediaLayout::Player => MediaNavigationPosition::Hidden,
        MediaLayout::Inline | MediaLayout::Stacked | MediaLayout::Showcase => {
            MediaNavigationPosition::WithControls
        }
    }
}

#[must_use]
pub const fn default_card_height_for_layout(layout: MediaLayout) -> i32 {
    match layout {
        MediaLayout::Carousel => 72,
        MediaLayout::Inline => 92,
        MediaLayout::Stacked => 112,
        MediaLayout::Showcase => 96,
        MediaLayout::Player => 208,
    }
}

fn default_browser_tokens() -> Vec<String> {
    // Tokens are lowercase because runtime matching normalizes player names before comparison
    vec![
        "firefox".to_string(),
        "librewolf".to_string(),
        "waterfox".to_string(),
        "floorp".to_string(),
        "brave".to_string(),
        "chromium".to_string(),
        "chrome".to_string(),
        "vivaldi".to_string(),
        "edge".to_string(),
        "opera".to_string(),
        "epiphany".to_string(),
        "midori".to_string(),
        "zen".to_string(),
    ]
}
