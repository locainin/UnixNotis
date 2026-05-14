//! Shared GTK CSS capability checks

pub const GTK_CSS_CUSTOM_PROPERTIES_MIN_VERSION_LABEL: &str = "GTK 4.16+";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GtkCssFeatures {
    // Newer GTK builds can expand var() and custom properties
    pub custom_properties: bool,
}

impl GtkCssFeatures {
    pub const fn supports_modern_theme_tokens(self) -> bool {
        self.custom_properties
    }
}

pub const fn gtk_css_features_for_version(major: u32, minor: u32) -> GtkCssFeatures {
    // GTK 4.16 added custom properties and var()
    GtkCssFeatures {
        custom_properties: major > 4 || (major == 4 && minor >= 16),
    }
}

pub fn gtk_css_features_from_version_string(version: &str) -> Option<GtkCssFeatures> {
    // pkg-config output can include patch and distro suffixes, but only major/minor matter here
    let (major, minor) = parse_major_minor(version)?;
    Some(gtk_css_features_for_version(major, minor))
}

fn parse_major_minor(version: &str) -> Option<(u32, u32)> {
    let mut parts = version.split('.');
    let major = parse_version_part(parts.next()?)?;
    let minor = parse_version_part(parts.next()?)?;
    Some((major, minor))
}

fn parse_version_part(part: &str) -> Option<u32> {
    // Stop at the first non-digit so values like 4.16.0-2 still parse cleanly
    let digits = part
        .trim()
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    (!digits.is_empty())
        .then(|| digits.parse::<u32>().ok())
        .flatten()
}

#[cfg(test)]
#[path = "tests/features.rs"]
mod tests;
