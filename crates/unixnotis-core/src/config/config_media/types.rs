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
    /// Centered player layout with cover art above the title and transport dock
    Player,
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
