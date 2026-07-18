use std::path::PathBuf;

use super::IconAssetError;

#[test]
fn errors_keep_security_reason_and_safe_path_context() {
    let error = IconAssetError::EmbeddedSvgImage(PathBuf::from("assets/icon.svg"));

    assert_eq!(
        error.to_string(),
        "SVG icon_asset must not embed or reference secondary images: assets/icon.svg"
    );
}
