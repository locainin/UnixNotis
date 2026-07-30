use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use image::{ImageBuffer, ImageFormat, Rgba};

use super::{decode_icon_file, MAX_ICON_SOURCE_DIMENSION};

fn test_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be valid")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "unixnotis-popups-{name}-{}-{nonce}.png",
        std::process::id()
    ))
}

#[test]
fn decode_icon_file_rejects_large_dimensions_before_full_decode() {
    let path = test_path("oversized");
    let image = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_pixel(
        MAX_ICON_SOURCE_DIMENSION + 1,
        8,
        Rgba([0, 0, 0, 255]),
    );
    image
        .save_with_format(&path, ImageFormat::Png)
        .expect("save image");

    let Err(err) = decode_icon_file(&path, 20) else {
        panic!("oversized image should fail")
    };
    assert!(
        err.contains("decode limit")
            || err.contains("dimensions exceed")
            || err.contains("exceeds limit"),
        "unexpected error: {err}"
    );
    let _ = fs::remove_file(&path);
}

#[test]
fn decode_icon_file_scales_to_requested_size() {
    let path = test_path("scale");
    let image = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_pixel(64, 32, Rgba([1, 2, 3, 255]));
    image
        .save_with_format(&path, ImageFormat::Png)
        .expect("save image");

    let decoded = decode_icon_file(&path, 20).expect("decode icon");

    assert!(decoded.width <= 20);
    assert!(decoded.height <= 20);
    assert_eq!(decoded.stride, decoded.width.saturating_mul(4));
    let _ = fs::remove_file(&path);
}

#[test]
fn decode_icon_file_rejects_non_files() {
    let path = test_path("not-file");
    fs::create_dir(&path).expect("create temp dir");

    let Err(err) = decode_icon_file(&path, 20) else {
        panic!("directory should fail")
    };
    assert_eq!(err, "icon path is not a regular file");
    let _ = fs::remove_dir(&path);
}
