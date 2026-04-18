//! Popup text sizing and empty-row handling
//!
//! Keeps label rules in one place so summary and body rows stay consistent

use std::borrow::Cow;

use gtk::prelude::*;

// Header/app title stays single-line and clipped at this length
pub(super) const POPUP_APP_MAX_CHARS: usize = 40;
// Summary is visually dominant but still bounded to avoid tall cards
pub(super) const POPUP_SUMMARY_MAX_CHARS: usize = 120;
// Body keeps enough context while preventing oversized popup growth
pub(super) const POPUP_BODY_MAX_CHARS: usize = 320;
// Action labels stay short so button row width remains predictable
pub(super) const POPUP_ACTION_LABEL_MAX_CHARS: usize = 14;

pub(super) struct OptionalLabelState<'a> {
    // Empty rows should disappear instead of leaving stray spacing behind
    pub(super) visible: bool,
    // Reuse borrowed text when possible so empty checks stay cheap
    pub(super) text: Cow<'a, str>,
}

pub(super) fn update_optional_label(label: &gtk::Label, text: &str, max_chars: usize) {
    // Build the layout decision first so empty-text handling stays identical
    // for both summary and body rows
    let state = optional_label_state(text, max_chars);
    // Hidden labels collapse their space in the popup box
    label.set_visible(state.visible);
    // Text assignment happens after the visibility decision so empty rows stay blank
    label.set_text(state.text.as_ref());
}

pub(super) fn optional_label_state(text: &str, max_chars: usize) -> OptionalLabelState<'_> {
    if !has_visible_text(text) {
        // Empty text rows stay hidden so the card does not keep dead spacing
        return OptionalLabelState {
            visible: false,
            text: Cow::Borrowed(""),
        };
    }
    let text = clamp_label_text(text, max_chars);
    OptionalLabelState {
        // Clamped-empty text should collapse the row the same way raw empty text does
        visible: has_visible_text(text.as_ref()),
        // Clamp before the label sees the text so layout work stays bounded
        text,
    }
}

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
