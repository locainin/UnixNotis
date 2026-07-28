//! Shared text limits that keep both notification surfaces bounded

use std::borrow::Cow;

pub const APP_LABEL_MAX_CHARS: usize = 64;
pub const SUMMARY_LABEL_MAX_CHARS: usize = 120;
pub const BODY_LABEL_MAX_CHARS: usize = 320;
pub const ACTION_LABEL_MAX_CHARS: usize = 20;

#[must_use]
pub fn has_visible_text(text: &str) -> bool {
    text.chars().any(|character| !character.is_whitespace())
}

#[must_use]
pub fn clamp_label_text(text: &str, max_chars: usize) -> Cow<'_, str> {
    if max_chars == 0 {
        return Cow::Borrowed("");
    }
    // Character boundaries retain valid UTF-8 for untrusted notification strings
    for (characters, (index, _)) in text.char_indices().enumerate() {
        if characters == max_chars {
            let mut clamped = String::with_capacity(index + 3);
            clamped.push_str(&text[..index]);
            clamped.push('…');
            return Cow::Owned(clamped);
        }
    }
    Cow::Borrowed(text)
}
