use std::path::{Path, PathBuf};

use super::{asset_path_reason, has_css_extension, local_file_url_path};

#[test]
fn css_path_helpers_accept_css_and_local_file_urls_only() {
    assert!(has_css_extension(Path::new("PANEL.CSS")));
    assert!(!has_css_extension(Path::new("panel.toml")));
    assert_eq!(
        local_file_url_path("file://localhost/config/image.png"),
        Some(PathBuf::from("/config/image.png"))
    );
    assert_eq!(
        local_file_url_path("https://example.invalid/image.png"),
        None
    );
}

#[test]
fn asset_path_reason_rejects_only_lexical_root_escapes() {
    let root = Path::new("/config/unixnotis");

    assert_eq!(
        asset_path_reason(root, &root.join("assets/image.png")),
        None
    );
    assert!(asset_path_reason(root, Path::new("/config/outside.png")).is_some());
}
