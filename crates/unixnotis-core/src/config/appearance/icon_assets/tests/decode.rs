use std::path::Path;

use super::{validate_dimensions, validate_icon_asset_contents};
use crate::config::{AssetPolicy, IconAssetError};

#[test]
fn dimension_checks_reject_zero_width_and_pixel_overflow() {
    let policy = AssetPolicy {
        max_width: 100,
        max_height: 100,
        max_pixels: 50,
        ..AssetPolicy::default()
    };

    assert!(matches!(
        validate_dimensions(Path::new("icon.png"), 0, 1, policy),
        Err(IconAssetError::Dimensions { .. })
    ));
    assert!(matches!(
        validate_dimensions(Path::new("icon.png"), 8, 8, policy),
        Err(IconAssetError::Dimensions { .. })
    ));
}

#[test]
fn content_validation_rejects_unsupported_extensions_before_decode() {
    assert!(matches!(
        validate_icon_asset_contents("assets/icon.sh", b"not executable"),
        Err(IconAssetError::UnsupportedExtension(_))
    ));
}
