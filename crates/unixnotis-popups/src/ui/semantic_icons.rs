//! Daemon-controlled badges for uncertain and non-application identities

use gtk::prelude::*;

use super::entry::TrustLevel;

const RESOURCE_ROOT: &str = "/com/unixnotis/Popups/icons";

pub(super) fn build_semantic_badge(level: TrustLevel, size: i32) -> Option<gtk::Image> {
    crate::app::resources::register().ok()?;
    let file = match level {
        // Verified applications always retain the authenticated desktop badge
        TrustLevel::Verified => return None,
        TrustLevel::Unverified => "unixnotis-app-unknown-symbolic.svg",
        TrustLevel::Suspicious => "unixnotis-shield-warning-symbolic.svg",
        TrustLevel::System => "unixnotis-terminal-symbolic.svg",
    };
    let image = gtk::Image::from_resource(&format!("{RESOURCE_ROOT}/{file}"));
    let size = size.max(1);
    image.set_pixel_size(size);
    image.set_size_request(size, size);
    Some(image)
}

#[cfg(test)]
#[path = "tests/semantic_icons.rs"]
mod tests;
