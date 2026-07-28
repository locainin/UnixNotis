//! Popup text sizing and empty-row handling
//!
//! Keeps label rules in one place so summary and body rows stay consistent

use std::borrow::Cow;

// Header/app title stays single-line and clipped at this length
pub(super) const POPUP_APP_MAX_CHARS: usize = 64;
// Summary is visually dominant but still bounded to avoid tall cards
pub(super) const POPUP_SUMMARY_MAX_CHARS: usize = 120;
// Body keeps enough context while preventing oversized popup growth
pub(super) const POPUP_BODY_MAX_CHARS: usize = 320;
// Action labels stay short so button row width remains predictable
pub(super) const POPUP_ACTION_LABEL_MAX_CHARS: usize = 14;

pub(super) fn has_visible_text(text: &str) -> bool {
    // Visibility depends on real content, not just raw string length
    // Space-only strings count as empty for popup layout purposes
    text.chars().any(|ch| !ch.is_whitespace())
}

pub(super) fn clamp_label_text(text: &str, max_chars: usize) -> Cow<'_, str> {
    if max_chars == 0 {
        // Zero means the caller wants an intentionally blank label
        return Cow::Borrowed("");
    }
    // char_indices preserves UTF-8 boundaries during truncation
    for (chars, (idx, _)) in text.char_indices().enumerate() {
        if chars == max_chars {
            // Keep one glyph slot for the ellipsis instead of splitting the codepoint
            let mut clamped = String::with_capacity(idx + 3);
            clamped.push_str(&text[..idx]);
            clamped.push('…');
            return Cow::Owned(clamped);
        }
    }
    // Borrow the original text when no clamp is needed
    Cow::Borrowed(text)
}

#[cfg(test)]
#[path = "tests/labels.rs"]
mod tests;
