//! Media widget configuration and shared layout defaults
//!
//! Keeps the media surface in one place so the center, tools, and tests all
//! reason about the same runtime contract

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct MediaConfig {
    /// Enable the media widget in the notification center
    pub enabled: bool,
    /// Structural preset for the media card
    pub layout: MediaLayout,
    /// Include web browser media players
    pub include_browsers: bool,
    /// Browser-identifying substrings for MPRIS bus names or identities
    pub browser_tokens: Vec<String>,
    /// Characters allowed before marquee scrolling begins
    pub title_char_limit: usize,
    /// Show the source label row above the title
    pub show_source: bool,
    /// Keep the source label visible when only one player exists
    pub show_source_when_single_player: bool,
    /// Show the player position text alongside the source label
    pub show_position: bool,
    /// Keep the position label visible when only one player exists
    pub show_position_when_single_player: bool,
    /// Show the title lane at all
    pub show_title: bool,
    /// Show the artist lane at all
    pub show_artist: bool,
    /// Show album artwork when present
    pub show_art: bool,
    /// Show transport buttons
    pub show_controls: bool,
    /// Show next and previous player navigation
    pub show_navigation: bool,
    /// How missing titles should be filled
    pub title_fallback: MediaTitleFallback,
    /// How the player position should be rendered
    pub position_format: MediaPositionFormat,
    /// Lowercase substring aliases applied to player identity or bus names
    pub source_aliases: BTreeMap<String, String>,
    /// Override the art slot placement on top of the structural preset
    pub art_position: MediaArtPosition,
    /// Override the control cluster placement on top of the structural preset
    pub controls_position: MediaControlsPosition,
    /// Override the navigation placement on top of the structural preset
    pub navigation_position: MediaNavigationPosition,
    /// Preferred art edge length in pixels
    pub art_size_px: i32,
    /// Minimum width budget reserved for the title lane
    pub text_width_floor_px: i32,
    /// Optional exact card height override in pixels
    pub card_height_px: Option<i32>,
    /// Spacing between major media card sections
    pub content_spacing_px: i32,
    /// Spacing between transport buttons
    pub control_spacing_px: i32,
    /// Spacing between navigation buttons or between nav and controls
    pub navigation_spacing_px: i32,
    /// Allowlist of player identifiers or bus names
    #[serde(alias = "whitelist")]
    pub allowlist: Vec<String>,
    /// Denylist of player identifiers or bus names
    #[serde(alias = "blacklist")]
    pub denylist: Vec<String>,
    /// Controls which players may trigger remote media artwork fetches
    pub remote_art_policy: MediaRemoteArtPolicy,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MediaRemoteArtPolicy {
    /// Disable remote artwork fetches for every player
    Disabled,
    /// Allow remote artwork only for non-browser players
    NativeOnly,
    /// Allow remote artwork for browsers too
    BrowsersToo,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MediaLayout {
    /// Existing carousel layout with navigation buttons outside the card
    Carousel,
    /// Single card layout with nav buttons folded into the transport strip
    Inline,
    /// Vertical card layout with a separate control strip under the metadata row
    Stacked,
    /// Wide dashboard layout with a dedicated action rail on the right
    Showcase,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MediaTitleFallback {
    /// Missing titles fall back to the player identity
    Identity,
    /// Missing titles fall back to the artist name when present
    Artist,
    /// Missing titles stay blank
    Empty,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MediaPositionFormat {
    /// Show the active slot and total count as `current/total`
    Fraction,
    /// Show only the active slot number
    Current,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MediaArtPosition {
    /// Use the preset-defined placement
    Auto,
    /// Keep artwork at the start of the card content
    Start,
    /// Move artwork above the text and controls
    Top,
    /// Remove artwork from the shell entirely
    Hidden,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MediaControlsPosition {
    /// Use the preset-defined placement
    Auto,
    /// Keep controls beside the main text lane
    Inline,
    /// Place controls under the main text lane
    Bottom,
    /// Move controls into a separate side rail
    Side,
    /// Remove controls from the shell entirely
    Hidden,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MediaNavigationPosition {
    /// Use the preset-defined placement
    Auto,
    /// Keep player navigation outside the card shell
    External,
    /// Keep navigation grouped with the transport area
    WithControls,
    /// Remove player navigation from the shell entirely
    Hidden,
}

impl Default for MediaRemoteArtPolicy {
    fn default() -> Self {
        Self::NativeOnly
    }
}

impl Default for MediaLayout {
    fn default() -> Self {
        Self::Carousel
    }
}

impl Default for MediaTitleFallback {
    fn default() -> Self {
        Self::Identity
    }
}

impl Default for MediaPositionFormat {
    fn default() -> Self {
        Self::Fraction
    }
}

impl Default for MediaArtPosition {
    fn default() -> Self {
        Self::Auto
    }
}

impl Default for MediaControlsPosition {
    fn default() -> Self {
        Self::Auto
    }
}

impl Default for MediaNavigationPosition {
    fn default() -> Self {
        Self::Auto
    }
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            layout: MediaLayout::Carousel,
            include_browsers: true,
            browser_tokens: vec![
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
            ],
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
            source_aliases: BTreeMap::new(),
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

impl MediaConfig {
    pub fn effective_art_position(&self) -> MediaArtPosition {
        if !self.show_art {
            return MediaArtPosition::Hidden;
        }
        match self.art_position {
            MediaArtPosition::Auto => default_art_position_for_layout(self.layout),
            position => position,
        }
    }

    pub fn effective_controls_position(&self) -> MediaControlsPosition {
        if !self.show_controls {
            return MediaControlsPosition::Hidden;
        }
        match self.controls_position {
            MediaControlsPosition::Auto => default_controls_position_for_layout(self.layout),
            position => position,
        }
    }

    pub fn effective_navigation_position(&self) -> MediaNavigationPosition {
        if !self.show_navigation {
            return MediaNavigationPosition::Hidden;
        }
        match self.navigation_position {
            MediaNavigationPosition::Auto => default_navigation_position_for_layout(self.layout),
            position => position,
        }
    }

    pub fn effective_card_height_px(&self) -> i32 {
        self.card_height_px
            .unwrap_or_else(|| default_card_height_for_layout(self.layout))
    }
}

pub fn default_art_position_for_layout(layout: MediaLayout) -> MediaArtPosition {
    match layout {
        MediaLayout::Stacked => MediaArtPosition::Top,
        MediaLayout::Carousel | MediaLayout::Inline | MediaLayout::Showcase => {
            MediaArtPosition::Start
        }
    }
}

pub fn default_controls_position_for_layout(layout: MediaLayout) -> MediaControlsPosition {
    match layout {
        MediaLayout::Carousel => MediaControlsPosition::Inline,
        MediaLayout::Inline | MediaLayout::Stacked => MediaControlsPosition::Bottom,
        MediaLayout::Showcase => MediaControlsPosition::Side,
    }
}

pub fn default_navigation_position_for_layout(layout: MediaLayout) -> MediaNavigationPosition {
    match layout {
        MediaLayout::Carousel => MediaNavigationPosition::External,
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
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MediaArtPosition, MediaConfig, MediaControlsPosition, MediaLayout, MediaNavigationPosition,
    };

    #[test]
    fn preset_defaults_stay_stable() {
        let mut config = MediaConfig::default();
        config.layout = MediaLayout::Carousel;
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
    fn hidden_flags_override_position_preferences() {
        let mut config = MediaConfig::default();
        config.show_art = false;
        config.show_controls = false;
        config.show_navigation = false;
        config.art_position = MediaArtPosition::Top;
        config.controls_position = MediaControlsPosition::Side;
        config.navigation_position = MediaNavigationPosition::External;

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
}
