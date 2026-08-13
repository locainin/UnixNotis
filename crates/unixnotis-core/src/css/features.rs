//! Shared GTK CSS capability checks

pub const GTK_MIN_VERSION_LABEL: &str = "GTK 4.18+";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GtkCssFeatures {
    // The installer uses this capability to enforce the supported baseline
    pub custom_properties: bool,
}

#[must_use]
pub const fn gtk_css_features_for_version(major: u32, minor: u32) -> GtkCssFeatures {
    // GTK 4.18 is the common baseline for CSS variables and popup Wayland APIs
    GtkCssFeatures {
        custom_properties: major > 4 || (major == 4 && minor >= 18),
    }
}

#[must_use]
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
    // Stop at the first non-digit so values like 4.18.0-2 still parse cleanly
    let digits = part
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    (!digits.is_empty())
        .then(|| digits.parse::<u32>().ok())
        .flatten()
}

#[cfg(test)]
#[path = "tests/features.rs"]
mod tests;
