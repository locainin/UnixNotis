//! Bounds for untrusted notification payload data
//!
//! Keeping limits in one file makes audits and tuning easier

pub(super) const MAX_APP_NAME_BYTES: usize = 256;
// Icon names/paths can be longer than app names, but still need a hard cap
pub(super) const MAX_APP_ICON_BYTES: usize = 1024;
// Summary is shown prominently, so keep it short and bounded
pub(super) const MAX_SUMMARY_BYTES: usize = 1024;
// Body can be larger, but still needs a strict upper bound
pub(super) const MAX_BODY_BYTES: usize = 16 * 1024;
// Category is used for grouping and rules, so keep values compact
pub(super) const MAX_CATEGORY_BYTES: usize = 256;
// Keep action rows compact so one notification cannot stretch list layout
// This limit is shared by popup and center action rendering expectations
pub(super) const MAX_ACTIONS: usize = 8;
// Action keys are internal identifiers
pub(super) const MAX_ACTION_KEY_BYTES: usize = 128;
// Action labels are user-facing button text
pub(super) const MAX_ACTION_LABEL_BYTES: usize = 256;
// Limit hint map size so map copies stay cheap
pub(super) const MAX_HINT_ENTRIES: usize = 16;
// Hint keys are short protocol labels
pub(super) const MAX_HINT_KEY_BYTES: usize = 64;
// String hints can be descriptive, but still capped for memory safety
pub(super) const MAX_HINT_STRING_BYTES: usize = 2048;
