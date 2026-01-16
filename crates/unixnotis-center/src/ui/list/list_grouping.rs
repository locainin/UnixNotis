//! Grouping key normalization and list consistency helpers.

use std::borrow::Cow;
use std::rc::Rc;

use super::NotificationList;

impl NotificationList {
    pub(super) fn intern_key(&mut self, key: &str) -> Rc<str> {
        let normalized = self.normalize_group_key(key);
        if let Some(value) = self.interned.get(normalized.as_ref()) {
            return value.clone();
        }
        // Normalize app names to avoid duplicate groups from case/whitespace variations.
        let value: Rc<str> = Rc::from(normalized.as_ref());
        self.interned.insert(value.clone());
        value
    }

    pub(super) fn normalize_group_key<'a>(&self, key: &'a str) -> Cow<'a, str> {
        // Trim outer whitespace to avoid duplicate stacks from padded app names.
        let trimmed = key.trim();
        if trimmed.is_empty() {
            return Cow::Borrowed("");
        }
        let mut normalized = String::new();
        // Track normalization to avoid allocations when the key is already clean.
        let mut changed = false;
        for ch in trimmed.chars() {
            if is_ignorable_group_char(ch) {
                // Strip invisible characters to keep visually identical names grouped.
                changed = true;
                continue;
            }
            if ch.is_ascii_uppercase() {
                // ASCII-only casing keeps stable group keys without locale-dependent transforms.
                normalized.push(ch.to_ascii_lowercase());
                changed = true;
            } else {
                normalized.push(ch);
            }
        }
        if normalized.is_empty() {
            return Cow::Borrowed("");
        }
        if changed {
            return Cow::Owned(normalized);
        }
        // Trim-only normalization keeps display text stable while grouping remains consistent.
        Cow::Borrowed(trimmed)
    }

    pub(super) fn expected_list_len(&self) -> usize {
        // Sum group block sizes to mirror the visible list length (headers + rows + ghosts).
        self.group_order
            .iter()
            .filter_map(|key| self.grouped_cache.get(key).map(|ids| (key, ids)))
            .map(|(key, ids)| self.group_block_len(key, ids))
            .sum()
    }
}

fn is_ignorable_group_char(ch: char) -> bool {
    // Strip control/zero-width characters to keep grouping stable for visually identical names.
    ch.is_control()
        || matches!(
            ch,
            '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}'
        )
}
