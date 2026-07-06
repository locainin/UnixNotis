//! Widget icon image construction with asset fallback

use tracing::warn;
use unixnotis_core::IconAssetResolver;

pub(super) fn image_from_icon_config(
    resolver: &IconAssetResolver,
    label: &str,
    icon_name: Option<&str>,
    icon_asset: Option<&str>,
) -> Option<gtk::Image> {
    if let Some(asset) = icon_asset.filter(|value| !value.trim().is_empty()) {
        match resolver.resolve_icon_asset_path(asset) {
            Ok(path) => {
                // File-backed images bypass icon-theme lookup entirely
                let image = gtk::Image::from_file(path);
                return Some(image);
            }
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
    Some(gtk::Image::from_icon_name(icon_name))
}
