// These values mirror the shipped theme and widget layout defaults
pub(super) const WIDTH_WARNING_TOLERANCE_PX: i32 = 8;
pub(super) const HEIGHT_WARNING_TOLERANCE_PX: i32 = 12;

// Fixed-grid widgets share spacing and fallback content widths
pub(super) const TOGGLE_GRID_SPACING_PX: i32 = 8;
pub(super) const TOGGLE_FALLBACK_CONTENT_WIDTH_PX: i32 = 80;

pub(super) const STAT_GRID_SPACING_PX: i32 = 8;
pub(super) const STAT_FALLBACK_CONTENT_WIDTH_PX: i32 = 96;

pub(super) const CARD_GRID_SPACING_PX: i32 = 8;
pub(super) const CARD_FALLBACK_CONTENT_WIDTH_PX: i32 = 104;

// Media keeps separate reserves because each layout spends width differently
pub(super) const MEDIA_TEXT_WIDTH_FLOOR_PX: i32 = 140;
pub(super) const MEDIA_NAV_FALLBACK_WIDTH_PX: i32 = 22;
pub(super) const MEDIA_ART_FALLBACK_WIDTH_PX: i32 = 50;
pub(super) const MEDIA_ART_FRAME_FALLBACK_WIDTH_PX: i32 = 54;
pub(super) const MEDIA_BUTTON_FALLBACK_WIDTH_PX: i32 = 28;
// Height warnings only trust explicit theme pressure by default
// Labels and buttons stay flexible until css adds a real min-height or fixed height
pub(super) const MEDIA_META_LABEL_FALLBACK_HEIGHT_PX: i32 = 0;
pub(super) const MEDIA_TITLE_FALLBACK_HEIGHT_PX: i32 = 0;
pub(super) const MEDIA_ARTIST_FALLBACK_HEIGHT_PX: i32 = 0;
pub(super) const MEDIA_NAV_FALLBACK_HEIGHT_PX: i32 = 0;
pub(super) const MEDIA_BUTTON_FALLBACK_HEIGHT_PX: i32 = 0;
pub(super) const MEDIA_TEXT_ROW_SPACING_PX: i32 = 2;
