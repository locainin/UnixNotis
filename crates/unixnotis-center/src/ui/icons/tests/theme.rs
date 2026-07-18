use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    expand_rgb_to_rgba, resolve_icon_source, theme_path_uses_worker, worker_decodes_theme_path,
};
use unixnotis_core::ImageData;

#[test]
fn expand_rgb_to_rgba_appends_alpha() {
    let data = ImageData {
        width: 2,
        height: 1,
        rowstride: 0,
        has_alpha: false,
        bits_per_sample: 8,
        channels: 3,
        data: vec![10, 20, 30, 40, 50, 60],
    };
    let (expanded, stride) = expand_rgb_to_rgba(&data).expect("rgb expansion");
    assert_eq!(stride, 8);
    assert_eq!(expanded, vec![10, 20, 30, 255, 40, 50, 60, 255]);
}

#[test]
fn theme_worker_accepts_only_its_bounded_raster_formats() {
    for path in ["icon.png", "icon.JPEG", "icon.webp", "icon.tiff"] {
        assert!(worker_decodes_theme_path(Path::new(path)), "{path}");
    }
    for path in ["icon.svg", "icon.svgz", "icon.xpm", "icon"] {
        assert!(!worker_decodes_theme_path(Path::new(path)), "{path}");
    }
}

#[cfg(unix)]
#[test]
fn theme_worker_path_requires_a_regular_non_symlink_raster_name() {
    use std::os::unix::fs::symlink;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "unixnotis-center-theme-path-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create theme path root");
    let raster = root.join("icon.png");
    let link = root.join("linked.png");
    let vector = root.join("icon.svg");
    fs::write(&raster, b"raster fixture").expect("write raster fixture");
    fs::write(&vector, b"vector fixture").expect("write vector fixture");
    symlink(&raster, &link).expect("create raster link");

    assert!(theme_path_uses_worker(&raster));
    assert!(!theme_path_uses_worker(&link));
    assert!(!theme_path_uses_worker(&vector));

    fs::remove_dir_all(root).expect("remove theme path root");
}

#[gtk::test]
fn standard_theme_icon_resolves_to_a_renderable_source() {
    let source = resolve_icon_source("folder", 24, 1)
        .or_else(|| resolve_icon_source("folder-symbolic", 24, 1));

    assert!(source.is_some());
}

#[test]
fn expand_rgb_to_rgba_honors_row_padding() {
    let data = ImageData {
        width: 2,
        height: 1,
        rowstride: 8,
        has_alpha: false,
        bits_per_sample: 8,
        channels: 3,
        data: vec![1, 2, 3, 4, 5, 6, 99, 100],
    };
    let (expanded, stride) = expand_rgb_to_rgba(&data).expect("rgb expansion");
    assert_eq!(stride, 8);
    assert_eq!(expanded, vec![1, 2, 3, 255, 4, 5, 6, 255]);
}
