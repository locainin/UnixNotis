use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    resolve_icon_asset_path, resolve_icon_asset_path_with_policy, validate_icon_asset_reference,
    AssetPolicy, IconAssetError, IconAssetResolver,
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
        let path = std::env::temp_dir().join(format!(
            "unixnotis-icon-assets-{}-{}-{}",
            name, stamp, serial
        ));
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

    let resolved = resolver
        .resolve_icon_asset_path("assets/ram.svg")
        .expect("resolve with resolver object");

    assert_eq!(resolved, expected);
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
