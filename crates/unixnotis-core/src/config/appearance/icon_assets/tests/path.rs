use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    normalize_icon_asset_relative_path, read_icon_asset_beneath_root,
    validate_icon_asset_extension, validate_icon_asset_reference,
};
use crate::config::{AssetPolicy, IconAssetError};

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("unixnotis-icon-path-{stamp}-{serial}"));
        std::fs::create_dir_all(&path).expect("create config root");
        Self(path)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn normalization_accepts_clean_relative_paths_and_removes_dot_segments() {
    assert_eq!(
        normalize_icon_asset_relative_path("./assets/icon.svg").expect("normalize icon"),
        PathBuf::from("assets/icon.svg")
    );
}

#[test]
fn normalization_rejects_urls_absolute_paths_and_parent_traversal() {
    assert!(matches!(
        normalize_icon_asset_relative_path("https://example.invalid/icon.svg"),
        Err(IconAssetError::Url(_))
    ));
    assert!(matches!(
        normalize_icon_asset_relative_path("/tmp/icon.svg"),
        Err(IconAssetError::Absolute(_))
    ));
    assert!(matches!(
        normalize_icon_asset_relative_path("../icon.svg"),
        Err(IconAssetError::ParentTraversal(_))
    ));
}

#[test]
fn extension_policy_is_case_insensitive_and_data_only() {
    let policy = AssetPolicy::default();

    validate_icon_asset_extension(Path::new("assets/icon.SVG"), policy)
        .expect("uppercase SVG extension");
    assert!(validate_icon_asset_extension(Path::new("assets/icon.sh"), policy).is_err());
    assert!(validate_icon_asset_reference("assets/icon.webp").is_ok());
}

#[test]
fn descriptor_reader_accepts_the_exact_byte_limit() {
    let root = TempRoot::new();
    std::fs::write(root.0.join("icon.png"), b"1234").expect("write icon");
    let policy = AssetPolicy {
        max_bytes: 4,
        allowed_extensions: &["png"],
        ..AssetPolicy::default()
    };

    let bytes = read_icon_asset_beneath_root(&root.0, Path::new("icon.png"), policy)
        .expect("read exact-size icon");

    assert_eq!(bytes, b"1234");
}

#[cfg(unix)]
#[test]
fn descriptor_reader_rejects_a_fifo_without_waiting_for_a_writer() {
    use rustix::fs::{mknodat, FileType, Mode, CWD};

    let root = TempRoot::new();
    let fifo = root.0.join("icon.png");
    mknodat(
        CWD,
        &fifo,
        FileType::Fifo,
        Mode::from_bits_truncate(0o600),
        0,
    )
    .expect("create fifo");

    let error =
        read_icon_asset_beneath_root(&root.0, Path::new("icon.png"), AssetPolicy::default())
            .expect_err("fifo must not be read");

    assert!(matches!(error, IconAssetError::NotRegularFile(_)));
}
