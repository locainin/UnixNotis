use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::collect_existing_icon_assets;

#[test]
fn icon_asset_collection_keeps_only_existing_config_relative_files() {
    static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

    let serial = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "unixnotis-export-assets-{}-{serial}",
        std::process::id(),
    ));
    let assets = root.join("assets");
    fs::create_dir_all(&assets).expect("create export asset directory");
    fs::write(assets.join("cpu.png"), [0_u8]).expect("write regular icon fixture");
    fs::write(
        root.join("config.toml"),
        r#"
            [widgets.volume]
            icon_asset = "assets/cpu.png"

            [widgets.brightness]
            icon_asset = "assets/missing.png"
        "#,
    )
    .expect("write export config fixture");

    let found = collect_existing_icon_assets(&root.join("config.toml"), &root)
        .expect("collect configured icon assets");

    assert_eq!(found, vec![std::path::PathBuf::from("assets/cpu.png")]);
    fs::remove_dir_all(root).expect("remove export asset directory");
}
