//! GTK capability checks

use unixnotis_core::{
    gtk_css_features_from_version_string, GTK_CSS_CUSTOM_PROPERTIES_MIN_VERSION_LABEL,
};

use super::system::pkg_config_version;
use super::{CheckItem, CheckState};

pub(super) fn gtk4_css_features_check(pkg_config: &CheckItem) -> CheckItem {
    // Modern CSS support is additive, so older GTK builds should warn instead of fail
    match pkg_config_version("gtk4") {
        Ok(Some(version)) => match gtk_css_features_from_version_string(&version) {
            Some(features) if features.custom_properties => CheckItem::ok(
                "GTK4 CSS features",
                &format!("found {version}; modern css variables and var() are available"),
            ),
            Some(_) => CheckItem::warn(
                "GTK4 CSS features",
                &format!(
                    "found {version}; legacy theming still works, but modern css variables need {GTK_CSS_CUSTOM_PROPERTIES_MIN_VERSION_LABEL}"
                ),
            ),
            None => CheckItem::warn(
                "GTK4 CSS features",
                &format!("found {version}; css feature level could not be parsed"),
            ),
        },
        Ok(None) if pkg_config.state == CheckState::Fail => CheckItem::warn(
            "GTK4 CSS features",
            "pkg-config missing; cannot probe GTK4 css feature level",
        ),
        Ok(None) => CheckItem::warn(
            "GTK4 CSS features",
            "pkg-config gtk4 not found; modern css feature support is unknown",
        ),
        Err(err) => CheckItem::warn("GTK4 CSS features", &format!("check failed: {err}")),
    }
}

pub(super) fn gtk4_layer_shell_check(pkg_config: &CheckItem) -> CheckItem {
    match pkg_config_version("gtk4-layer-shell-0") {
        Ok(Some(version)) => CheckItem::ok("gtk4-layer-shell", &format!("found {version}")),
        Ok(None) if pkg_config.state == CheckState::Fail => CheckItem::fail(
            "gtk4-layer-shell",
            "pkg-config missing; cannot probe gtk4-layer-shell",
        ),
        Ok(None) => CheckItem::fail(
            "gtk4-layer-shell",
            // This package is required, so missing metadata stays a hard stop
            "pkg-config gtk4-layer-shell-0 not found; is gtk4-layer-shell installed?",
        ),
        Err(err) => CheckItem::fail("gtk4-layer-shell", &format!("check failed: {err}")),
    }
}

#[cfg(test)]
#[path = "tests/gtk.rs"]
mod tests;
