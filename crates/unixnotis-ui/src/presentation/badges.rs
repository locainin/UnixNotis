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
    let image = gtk::Image::new();
    apply_semantic_badge(&image, badge, size).then_some(image)
}

/// Applies one daemon-controlled symbolic icon to an existing reusable image widget
#[must_use]
pub fn apply_semantic_badge(image: &gtk::Image, badge: BadgePresentation, size: i32) -> bool {
    if register_semantic_badges().is_err() {
        return false;
    }
    let Some(display) = gtk::gdk::Display::default() else {
        return false;
    };
    let icon_theme = gtk::IconTheme::for_display(&display);
    // Named symbolic icons use GTK's recoloring path instead of raw resource paintables
    icon_theme.add_resource_path(RESOURCE_ROOT);
    let icon_name = match badge {
        // Verified applications retain the authenticated desktop badge
        BadgePresentation::AuthenticatedApplication => return false,
        BadgePresentation::UnknownApplication => "unixnotis-app-unknown-symbolic",
        BadgePresentation::SuspiciousApplication => "unixnotis-shield-warning-symbolic",
        BadgePresentation::CommandLine => "unixnotis-terminal-symbolic",
        BadgePresentation::System => "unixnotis-system-symbolic",
    };
    let size = size.max(1);
    image.set_paintable(None::<&gtk::gdk::Paintable>);
    image.set_icon_name(Some(icon_name));
    image.set_pixel_size(size);
    image.set_size_request(size, size);
    true
}

#[cfg(test)]
#[path = "tests/badges.rs"]
mod tests;
