use super::types::{
    MediaArtPosition, MediaConfig, MediaControlsPosition, MediaLayout, MediaNavigationPosition,
    MediaPositionFormat, MediaRemoteArtPolicy, MediaTitleFallback,
};

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            layout: MediaLayout::Carousel,
            include_browsers: true,
            browser_tokens: default_browser_tokens(),
            title_char_limit: 32,
            show_source: true,
            show_source_when_single_player: true,
            show_position: true,
            show_position_when_single_player: false,
            show_title: true,
            show_artist: true,
            show_art: true,
            show_controls: true,
            show_navigation: true,
            title_fallback: MediaTitleFallback::Identity,
            position_format: MediaPositionFormat::Fraction,
            source_aliases: Default::default(),
            art_position: MediaArtPosition::Auto,
            controls_position: MediaControlsPosition::Auto,
            navigation_position: MediaNavigationPosition::Auto,
            art_size_px: 50,
            text_width_floor_px: 140,
            card_height_px: None,
            content_spacing_px: 10,
            control_spacing_px: 6,
            navigation_spacing_px: 6,
            allowlist: Vec::new(),
            denylist: vec!["playerctld".to_string()],
            // Browsers stay opt-in because webpage metadata can choose artwork URLs
            remote_art_policy: MediaRemoteArtPolicy::NativeOnly,
        }
    }
}

pub fn default_art_position_for_layout(layout: MediaLayout) -> MediaArtPosition {
    match layout {
        MediaLayout::Stacked | MediaLayout::Player => MediaArtPosition::Top,
        MediaLayout::Carousel | MediaLayout::Inline | MediaLayout::Showcase => {
            MediaArtPosition::Start
        }
    }
}

pub fn default_controls_position_for_layout(layout: MediaLayout) -> MediaControlsPosition {
    match layout {
        MediaLayout::Carousel => MediaControlsPosition::Inline,
        MediaLayout::Inline | MediaLayout::Stacked | MediaLayout::Player => {
            MediaControlsPosition::Bottom
        }
        MediaLayout::Showcase => MediaControlsPosition::Side,
    }
}

pub fn default_navigation_position_for_layout(layout: MediaLayout) -> MediaNavigationPosition {
    match layout {
        MediaLayout::Carousel => MediaNavigationPosition::External,
        MediaLayout::Player => MediaNavigationPosition::Hidden,
        MediaLayout::Inline | MediaLayout::Stacked | MediaLayout::Showcase => {
            MediaNavigationPosition::WithControls
        }
    }
}

pub fn default_card_height_for_layout(layout: MediaLayout) -> i32 {
    match layout {
        MediaLayout::Carousel => 72,
        MediaLayout::Inline => 92,
        MediaLayout::Stacked => 112,
        MediaLayout::Showcase => 96,
        MediaLayout::Player => 208,
    }
}

fn default_browser_tokens() -> Vec<String> {
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
