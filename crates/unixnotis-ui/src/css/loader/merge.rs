//! CSS override merge policy

pub(super) fn merge_css_with_overrides(contents: &str, fallback: &str, overrides: &str) -> String {
    if overrides.trim().is_empty() {
        return contents.to_string();
    }

    // User overrides are appended to untouched defaults and prepended to user-edited files
    if contents.trim() == fallback.trim() {
        format!("{contents}\n{overrides}")
    } else {
        format!("{overrides}\n{contents}")
    }
}

#[cfg(test)]
#[path = "../tests/loader/merge.rs"]
mod tests;
