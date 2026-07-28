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
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

pub(super) fn normalize_brand_name(value: &str) -> String {
    // UTS 39 skeletons collapse common cross-script lookalikes before comparison
    skeleton(value)
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_lowercase())
        .collect()
}
