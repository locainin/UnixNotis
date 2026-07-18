use std::path::PathBuf;

use super::{ExternalCssAssetRef, HostSpecificCssAssetRef};

#[test]
fn css_asset_findings_keep_source_reference_and_rewrite_separate() {
    let external = ExternalCssAssetRef {
        css_file: PathBuf::from("panel.css"),
        asset_ref: "../outside.png".to_string(),
        reason: "outside root".to_string(),
    };
    let host_specific = HostSpecificCssAssetRef {
        css_file: external.css_file.clone(),
        asset_ref: "file:///config/assets/image.png".to_string(),
        rewritten_ref: "assets/image.png".to_string(),
    };

    assert_eq!(external.css_file, PathBuf::from("panel.css"));
    assert_eq!(external.asset_ref, "../outside.png");
    assert_eq!(host_specific.rewritten_ref, "assets/image.png");
    assert_ne!(host_specific.asset_ref, host_specific.rewritten_ref);
}
