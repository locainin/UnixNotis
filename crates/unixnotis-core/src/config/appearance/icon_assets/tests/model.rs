use super::{
    AssetPolicy, ResolvedIconAsset, DEFAULT_ICON_ASSET_EXTENSIONS, DEFAULT_ICON_ASSET_MAX_BYTES,
    DEFAULT_ICON_ASSET_MAX_HEIGHT, DEFAULT_ICON_ASSET_MAX_PIXELS, DEFAULT_ICON_ASSET_MAX_WIDTH,
};

#[test]
fn default_policy_uses_all_published_limits() {
    let policy = AssetPolicy::default();

    assert_eq!(policy.max_bytes, DEFAULT_ICON_ASSET_MAX_BYTES);
    assert_eq!(policy.max_width, DEFAULT_ICON_ASSET_MAX_WIDTH);
    assert_eq!(policy.max_height, DEFAULT_ICON_ASSET_MAX_HEIGHT);
    assert_eq!(policy.max_pixels, DEFAULT_ICON_ASSET_MAX_PIXELS);
    assert_eq!(policy.allowed_extensions, DEFAULT_ICON_ASSET_EXTENSIONS);
}

#[test]
fn resolved_icon_keeps_alpha_representation_explicit() {
    let icon = ResolvedIconAsset {
        rgba: vec![1, 2, 3, 4],
        width: 1,
        height: 1,
        premultiplied_alpha: true,
    };

    assert_eq!(icon.rgba.len(), 4);
    assert!(icon.premultiplied_alpha);
}
