use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};
use unixnotis_core::IconAssetResolver;

use super::{resolve_icon_source, IconSource};

fn test_root() -> std::path::PathBuf {
    static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);
    let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "unixnotis-widget-icon-image-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("assets")).expect("create icon test root");
    root
}

fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let pixels = vec![255_u8; width as usize * height as usize * 4];
    PngEncoder::new(&mut bytes)
        .write_image(&pixels, width, height, ExtendedColorType::Rgba8)
        .expect("encode test png");
    bytes
}

#[test]
fn assets_use_fixed_slots_and_decode_failures_use_theme_fallback() {
    let root = test_root();
    fs::write(root.join("assets/corrupt.png"), b"not png").expect("write corrupt icon");
    let resolver = IconAssetResolver::new(&root);

    let source = resolve_icon_source(
        &resolver,
        "RAM",
        Some("drive-harddisk-symbolic"),
        Some("assets/corrupt.png"),
        16,
    )
    .expect("theme fallback source");
    assert!(matches!(source, IconSource::Theme(name) if name == "drive-harddisk-symbolic"));
    fs::write(root.join("assets/large.png"), png_bytes(512, 512)).expect("write icon");
    let resolver = IconAssetResolver::new(&root);

    let source = resolve_icon_source(
        &resolver,
        "RAM",
        Some("drive-harddisk-symbolic"),
        Some("assets/large.png"),
        16,
    )
    .expect("decoded asset source");
    let IconSource::Asset(asset) = source else {
        panic!("expected captured asset pixels");
    };
    assert_eq!((asset.width, asset.height), (16, 16));
    assert_eq!(asset.rgba.len(), 16 * 16 * 4);
    let _ = fs::remove_dir_all(root);
}
