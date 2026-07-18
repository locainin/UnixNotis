use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};

pub(super) fn test_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "unixnotis-center-icon-{name}-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create icon test root");
    root
}

pub(super) fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    let pixels = vec![0x7f; usize::try_from(width * height * 4).expect("small PNG size")];
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&pixels, width, height, ExtendedColorType::Rgba8)
        .expect("encode PNG");
    bytes
}
