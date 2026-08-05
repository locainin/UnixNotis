//! Desktop identifiers, aliases, and protected brand normalization

use unicode_security::skeleton;

pub(in crate::daemon::notifications::identity) fn normalize_desktop_id(value: &str) -> String {
    // Desktop hints commonly include an optional suffix and mixed case
    value
        .trim()
        .strip_suffix(".desktop")
        .unwrap_or_else(|| value.trim())
        .to_ascii_lowercase()
}

pub(in crate::daemon::notifications::identity) fn normalize_name(value: &str) -> String {
    // Punctuation and case do not create separate branding aliases
    let mut normalized = String::with_capacity(value.len());
    for character in value
        .chars()
        .filter(|character| character.is_alphanumeric())
    {
        normalized.extend(character.to_lowercase());
    }
    normalized
}

pub(super) fn normalize_brand_name(value: &str) -> String {
    // UTS 39 skeletons collapse common cross-script lookalikes before comparison
    let mut normalized = String::with_capacity(value.len());
    for character in skeleton(value).filter(char::is_ascii_alphanumeric) {
        normalized.push(character.to_ascii_lowercase());
    }
    normalized
}
