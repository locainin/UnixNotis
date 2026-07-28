//! Controlled security badges shared by every GTK notification client

use std::sync::OnceLock;

use gtk::prelude::*;

use super::BadgePresentation;

const RESOURCE_ROOT: &str = "/com/unixnotis/Ui/icons";

/// Registers bundled badge resources once for the current process
///
/// # Errors
///
/// Returns the original registration error when GTK cannot load the compiled resource
pub fn register_semantic_badges() -> Result<(), String> {
    static REGISTRATION: OnceLock<Result<(), String>> = OnceLock::new();
    // One cached result keeps repeated GTK startup and test initialization deterministic
    REGISTRATION
        .get_or_init(|| {
            gtk::gio::resources_register_include!("unixnotis-ui.gresource")
                .map_err(|error| format!("register bundled UI resources: {error}"))
        })
        .clone()
}

/// Builds a daemon-controlled badge when authenticated application art is not allowed
#[must_use]
pub fn build_semantic_badge(badge: BadgePresentation, size: i32) -> Option<gtk::Image> {
    register_semantic_badges().ok()?;
    let file = match badge {
        // Verified applications retain the authenticated desktop badge
        BadgePresentation::AuthenticatedApplication => return None,
        BadgePresentation::UnknownApplication => "unixnotis-app-unknown-symbolic.svg",
        BadgePresentation::SuspiciousApplication => "unixnotis-shield-warning-symbolic.svg",
        BadgePresentation::CommandLine => "unixnotis-terminal-symbolic.svg",
        BadgePresentation::System => "unixnotis-system-symbolic.svg",
    };
    let image = gtk::Image::from_resource(&format!("{RESOURCE_ROOT}/{file}"));
    let size = size.max(1);
    image.set_pixel_size(size);
    image.set_size_request(size, size);
    Some(image)
}

#[cfg(test)]
#[path = "tests/badges.rs"]
mod tests;
