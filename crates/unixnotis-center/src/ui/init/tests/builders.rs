use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::builders::icon_resolver_for_widgets;

#[test]
fn widget_icon_resolver_anchors_assets_to_active_config_directory() {
    static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

    let serial = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "unixnotis-init-builder-{}-{serial}",
        std::process::id(),
    ));
    let asset_dir = root.join("assets");
    let asset = asset_dir.join("status.png");
    fs::create_dir_all(&asset_dir).expect("create asset directory");
    fs::write(&asset, [0_u8]).expect("write regular asset fixture");

    let resolver = icon_resolver_for_widgets(&root.join("config.toml"));
    let resolved_path = resolver
        .resolve_icon_asset_path("assets/status.png")
        .expect("asset should resolve below config directory");

    assert_eq!(resolved_path, asset);
    fs::remove_dir_all(root).expect("remove builder test directory");
}
