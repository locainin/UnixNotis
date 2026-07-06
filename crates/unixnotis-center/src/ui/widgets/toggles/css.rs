//! Toggle CSS token helpers
//!
//! Keeps CSS class-name normalization rules separate from widget wiring

/// Converts a configured toggle kind into a CSS-safe class suffix
pub(super) fn toggle_kind_css_class(kind: &str) -> Option<String> {
    super::super::kind_css::widget_kind_css_class("unixnotis-toggle-kind-", kind)
}

#[cfg(test)]
#[path = "tests/css.rs"]
mod tests;
