//! GTK capability checks

use unixnotis_core::{gtk_css_features_from_version_string, GTK_MIN_VERSION_LABEL};

use super::system::pkg_config_version;
use super::{CheckItem, CheckState};

pub(super) fn gtk4_css_features_check(pkg_config: &CheckItem) -> CheckItem {
    // The shipped CSS contract requires the common GTK 4.18 baseline
    match pkg_config_version("gtk4") {
        Ok(Some(version)) => match gtk_css_features_from_version_string(&version) {
            Some(features) if features.custom_properties => CheckItem::ok(
                "GTK4 (4.18+)",
                &format!("found {version}; custom properties and var() are available"),
            ),
            Some(_) => CheckItem::fail(
                "GTK4 (4.18+)",
                &format!("found {version}; {GTK_MIN_VERSION_LABEL} is required"),
            ),
            None => CheckItem::fail(
                "GTK4 (4.18+)",
                &format!("found {version}; GTK version could not be parsed"),
            ),
        },
        Ok(None) if pkg_config.state == CheckState::Fail => CheckItem::fail(
            "GTK4 (4.18+)",
            "pkg-config missing; GTK 4.18 or newer is required",
        ),
        Ok(None) => CheckItem::fail(
            "GTK4 (4.18+)",
            "pkg-config gtk4 not found; GTK 4.18 or newer is required",
        ),
        Err(err) => CheckItem::fail("GTK4 (4.18+)", &format!("check failed: {err}")),
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
