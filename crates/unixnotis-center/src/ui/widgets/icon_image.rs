//! Widget icon image construction with asset fallback

use gtk::prelude::WidgetExt;
use tracing::warn;
use unixnotis_core::{IconAssetResolver, ResolvedIconAsset};

enum IconSource {
    Asset(ResolvedIconAsset),
    Theme(String),
}

pub(super) fn image_from_icon_config(
    resolver: &IconAssetResolver,
    label: &str,
    icon_name: Option<&str>,
    icon_asset: Option<&str>,
    render_size: u32,
) -> Option<gtk::Image> {
    match resolve_icon_source(resolver, label, icon_name, icon_asset, render_size)? {
        IconSource::Asset(asset) => {
            // GTK receives captured pixels rather than reopening an attacker-changeable path
            let format = if asset.premultiplied_alpha {
                gtk::gdk::MemoryFormat::R8g8b8a8Premultiplied
            } else {
                gtk::gdk::MemoryFormat::R8g8b8a8
            };
            let bytes = gtk::glib::Bytes::from_owned(asset.rgba);
            let texture = gtk::gdk::MemoryTexture::new(
                asset.width as i32,
                asset.height as i32,
                format,
                &bytes,
                asset.width as usize * 4,
            );
            let image = gtk::Image::from_paintable(Some(&texture));
            image.set_size_request(render_size as i32, render_size as i32);
            Some(image)
        }
        IconSource::Theme(icon_name) => {
            let image = gtk::Image::from_icon_name(&icon_name);
            image.set_pixel_size(render_size as i32);
            Some(image)
        }
    }
}

fn resolve_icon_source(
    resolver: &IconAssetResolver,
    label: &str,
    icon_name: Option<&str>,
    icon_asset: Option<&str>,
    render_size: u32,
) -> Option<IconSource> {
    if let Some(asset) = icon_asset.filter(|value| !value.trim().is_empty()) {
        match resolver.resolve_icon_asset(asset, render_size) {
            Ok(asset) => return Some(IconSource::Asset(asset)),
            Err(err) => {
                warn!(
                    label = %label,
                    asset = %asset,
                    ?err,
                    "invalid widget icon asset; falling back to theme icon"
                );
            }
        }
    }

    let icon_name = icon_name?.trim();
    if icon_name.is_empty() {
        return None;
    }

    // Theme names remain the compatibility path for existing configs and fallback icons
    Some(IconSource::Theme(icon_name.to_string()))
}

#[cfg(test)]
#[path = "tests/icon_image.rs"]
mod tests;
