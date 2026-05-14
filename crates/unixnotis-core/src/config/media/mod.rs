//! Media widget configuration and shared layout defaults
//!
//! Keeps the media surface in one place so the center, tools, and tests all
//! reason about the same runtime contract

mod defaults;
mod effective;
mod types;

pub use self::defaults::{
    default_art_position_for_layout, default_card_height_for_layout,
    default_controls_position_for_layout, default_navigation_position_for_layout,
    DEFAULT_MEDIA_ART_SIZE_PX, DEFAULT_MEDIA_TEXT_WIDTH_FLOOR_PX,
};
pub use self::types::{
    MediaArtPosition, MediaConfig, MediaControlsPosition, MediaLayout, MediaNavigationPosition,
    MediaPositionFormat, MediaRemoteArtPolicy, MediaTitleFallback,
};
