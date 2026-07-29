//! Validation for caller-provided desktop identifiers

const MAX_DESKTOP_ID_BYTES: usize = 256;

pub(super) fn validate_desktop_id(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAX_DESKTOP_ID_BYTES
        || value.contains(['/', '\\', '\0'])
        || value.chars().any(char::is_control)
    {
        return None;
    }

    let value = value.strip_suffix(".desktop").unwrap_or(value);
    if value == "." || value == ".." || value.is_empty() {
        return None;
    }
    Some(value.to_string())
}
