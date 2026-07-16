use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};

use super::{
    resolve_icon_asset_path, resolve_icon_asset_path_with_policy, validate_dimensions,
    validate_icon_asset_contents, validate_icon_asset_reference, AssetPolicy, IconAssetError,
    IconAssetResolver,
};

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("unixnotis-icon-assets-{name}-{stamp}-{serial}"));
        std::fs::create_dir_all(&path).expect("create temp root");
        Self { path }
    }

    fn write(&self, relative: &str, bytes: &[u8]) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(&path, bytes).expect("write file");
        path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let pixels = vec![0_u8; width as usize * height as usize * 4];
    PngEncoder::new(&mut bytes)
        .write_image(&pixels, width, height, ExtendedColorType::Rgba8)
        .expect("encode png");
    bytes
}

#[test]
fn relative_svg_asset_resolves_inside_config_root() {
    let root = TempRoot::new("valid");
    let expected = root.write("assets/ram.svg", b"<svg/>");

    let resolved =
        resolve_icon_asset_path(&root.path, "assets/ram.svg").expect("resolve icon asset");

    assert_eq!(resolved, expected);
}

#[test]
fn resolver_object_applies_config_root_and_policy() {
    let root = TempRoot::new("resolver");
    let expected = root.write("assets/ram.svg", b"<svg/>");
    let resolver = IconAssetResolver::new(root.path.clone());

    let resolved_path = resolver
        .resolve_icon_asset_path("assets/ram.svg")
        .expect("resolve with resolver object");

    assert_eq!(resolved_path, expected);
}

#[test]
fn disabled_resolver_never_falls_back_to_the_process_directory() {
    let resolver = IconAssetResolver::disabled();

    assert!(matches!(
        resolver.resolve_icon_asset_path("assets/ram.svg"),
        Err(IconAssetError::Disabled)
    ));
}

#[test]
fn largest_valid_png_is_rendered_inside_requested_icon_slot() {
    let root = TempRoot::new("bounded-render");
    root.write("assets/large.png", &png_bytes(512, 512));
    let resolver = IconAssetResolver::new(root.path.clone());

    let rendered_icon = resolver
        .resolve_icon_asset("assets/large.png", 16)
        .expect("decode bounded icon");

    assert_eq!((rendered_icon.width, rendered_icon.height), (16, 16));
    assert_eq!(rendered_icon.rgba.len(), 16 * 16 * 4);
}

#[test]
fn decoded_dimensions_and_corrupt_signatures_are_rejected() {
    let oversized = png_bytes(513, 1);
    assert!(matches!(
        validate_icon_asset_contents("assets/wide.png", &oversized),
        Err(IconAssetError::Decode { .. } | IconAssetError::Dimensions { .. })
    ));
    assert!(matches!(
        validate_icon_asset_contents("assets/corrupt.png", b"not a png"),
        Err(IconAssetError::InvalidFormat(_))
    ));
}

#[test]
fn excessive_svg_dimensions_are_rejected_before_rendering() {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="513" height="1"/>"#;

    assert!(matches!(
        validate_icon_asset_contents("assets/wide.svg", svg),
        Err(IconAssetError::Dimensions { .. })
    ));
}

#[test]
fn dimension_policy_accepts_exact_limits_and_rejects_each_independent_boundary() {
    let policy = AssetPolicy::default();
    validate_dimensions(Path::new("icon.png"), 512, 512, policy).expect("exact limits");

    for (width, height) in [(0, 1), (1, 0), (513, 1), (1, 513)] {
        assert!(validate_dimensions(Path::new("icon.png"), width, height, policy).is_err());
    }

    let pixel_policy = AssetPolicy {
        max_width: 512,
        max_height: 512,
        max_pixels: 100,
        ..policy
    };
    assert!(validate_dimensions(Path::new("icon.png"), 11, 10, pixel_policy).is_err());
}

#[test]
fn content_size_limit_is_inclusive_and_svg_image_nodes_are_rejected() {
    let prefix = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>"#;
    let mut exact = prefix.to_vec();
    exact.resize(2_097_152, b' ');
    validate_icon_asset_contents("assets/exact.svg", &exact).expect("exact byte limit");

    let embedded = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><image href="/tmp/evil.png"/></svg>"#;
    assert!(matches!(
        validate_icon_asset_contents("assets/embedded.svg", embedded),
        Err(IconAssetError::EmbeddedSvgImage(_))
    ));
}

#[test]
fn replacing_a_previously_validated_path_does_not_reuse_old_file_bytes() {
    let root = TempRoot::new("replace-after-validation");
    let path = root.write("assets/icon.png", &png_bytes(1, 1));
    let resolver = IconAssetResolver::new(root.path.clone());
    resolver
        .resolve_icon_asset_path("assets/icon.png")
        .expect("initial path validation");
    std::fs::write(path, b"corrupt replacement").expect("replace icon bytes");

    assert!(resolver.resolve_icon_asset("assets/icon.png", 16).is_err());
}

#[test]
fn parent_traversal_is_rejected() {
    let root = TempRoot::new("parent");

    assert!(resolve_icon_asset_path(&root.path, "../ram.svg").is_err());
}

#[test]
fn absolute_path_is_rejected() {
    let root = TempRoot::new("absolute");

    assert!(resolve_icon_asset_path(&root.path, "/tmp/ram.svg").is_err());
}

#[test]
fn remote_url_is_rejected() {
    let root = TempRoot::new("url");

    for asset in [
        "https://example.com/ram.svg",
        "http://example.com/ram.svg",
        "file:///tmp/ram.svg",
        "file:/tmp/ram.svg",
        "s3://bucket/ram.svg",
    ] {
        assert!(
            matches!(
                resolve_icon_asset_path(&root.path, asset),
                Err(IconAssetError::Url(_))
            ),
            "asset should fail as URL: {asset}"
        );
    }
}

#[test]
fn missing_asset_returns_error() {
    let root = TempRoot::new("missing");

    assert!(resolve_icon_asset_path(&root.path, "assets/missing.svg").is_err());
}

#[test]
fn directory_asset_returns_error() {
    let root = TempRoot::new("directory");
    std::fs::create_dir_all(root.path.join("assets/dir.svg")).expect("create asset dir");

    assert!(resolve_icon_asset_path(&root.path, "assets/dir.svg").is_err());
}

#[cfg(unix)]
#[test]
fn executable_asset_returns_error() {
    use std::os::unix::fs::PermissionsExt;

    let root = TempRoot::new("executable");
    let asset = root.write("assets/run.svg", b"<svg/>");
    let mut permissions = std::fs::metadata(&asset).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&asset, permissions).expect("set executable");

    assert!(resolve_icon_asset_path(&root.path, "assets/run.svg").is_err());
}

#[test]
fn oversized_asset_returns_error() {
    let root = TempRoot::new("oversized");
    root.write("assets/huge.png", b"too large");
    let policy = AssetPolicy {
        max_bytes: 4,
        allowed_extensions: &["png"],
        ..AssetPolicy::default()
    };

    assert!(resolve_icon_asset_path_with_policy(&root.path, "assets/huge.png", policy).is_err());
}

#[test]
fn asset_at_exact_size_limit_is_allowed() {
    let root = TempRoot::new("exact-size");
    root.write("assets/exact.png", b"1234");
    let policy = AssetPolicy {
        max_bytes: 4,
        allowed_extensions: &["png"],
        ..AssetPolicy::default()
    };

    assert!(resolve_icon_asset_path_with_policy(&root.path, "assets/exact.png", policy).is_ok());
}

#[test]
fn allowed_extensions_pass_and_disallowed_extensions_fail() {
    let root = TempRoot::new("extensions");
    root.write("assets/icon.png", b"png");
    root.write("assets/icon.sh", b"sh");

    assert!(resolve_icon_asset_path(&root.path, "assets/icon.png").is_ok());
    assert!(resolve_icon_asset_path(&root.path, "assets/icon.sh").is_err());
}

#[cfg(unix)]
#[test]
fn symlink_escape_is_rejected() {
    use std::os::unix::fs::symlink;

    let root = TempRoot::new("symlink-root");
    let outside = TempRoot::new("symlink-outside");
    let outside_asset = outside.write("ram.svg", b"<svg/>");
    std::fs::create_dir_all(root.path.join("assets")).expect("create assets");
    symlink(outside_asset, root.path.join("assets/ram.svg")).expect("create symlink");

    assert!(resolve_icon_asset_path(&root.path, "assets/ram.svg").is_err());
}

#[test]
fn import_reference_validation_does_not_require_file_to_exist() {
    assert!(validate_icon_asset_reference("assets/missing.svg").is_ok());
    assert!(validate_icon_asset_reference("../evil.svg").is_err());
    assert!(validate_icon_asset_reference("assets/run.sh").is_err());
    assert!(
        validate_icon_asset_reference(Path::new("/tmp/ram.svg").to_string_lossy().as_ref())
            .is_err()
    );
}
