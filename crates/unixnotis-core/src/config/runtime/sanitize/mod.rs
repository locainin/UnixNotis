//! Runtime sanitization and validation for configuration values

mod media;
mod panel;
mod pipeline;
mod plugins;
mod refresh;
mod shell;
mod theme;

pub(in super::super) use pipeline::sanitize_config;
pub(super) use pipeline::{
    MAX_BORDER_WIDTH, MAX_CARD_HEIGHT, MAX_CARD_RADIUS, MAX_CORNER_CUT, MAX_MEDIA_ART_SIZE,
    MAX_MEDIA_TEXT_WIDTH_FLOOR, MAX_MEDIA_TITLE_CHAR_LIMIT, MAX_SPACING, MAX_WIDGET_COLUMNS,
    MIN_MEDIA_TEXT_WIDTH_FLOOR, MIN_MEDIA_TITLE_CHAR_LIMIT, MIN_WIDGET_COLUMNS,
};
pub use plugins::{MAX_CARD_WIDGETS, MAX_STAT_WIDGETS, MAX_TOGGLE_WIDGETS, MAX_TOTAL_WIDGETS};
