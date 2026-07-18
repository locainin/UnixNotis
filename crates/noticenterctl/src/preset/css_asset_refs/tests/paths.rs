use std::path::Path;

use super::{asset_path_reason, has_css_extension, read_css_file_bounded, MAX_CSS_FILE_BYTES};

#[test]
fn css_path_helpers_accept_css_extensions_case_insensitively() {
    assert!(has_css_extension(Path::new("PANEL.CSS")));
    assert!(!has_css_extension(Path::new("panel.toml")));
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

#[test]
fn bounded_css_reader_accepts_the_exact_file_size_limit() {
    let path = std::env::temp_dir().join(format!(
        "unixnotis-exact-css-{}-{}.css",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    std::fs::write(
        &path,
        vec![b' '; usize::try_from(MAX_CSS_FILE_BYTES).expect("limit fits usize")],
    )
    .expect("write exact-size CSS");

    let result = read_css_file_bounded(&path);
    let _ = std::fs::remove_file(&path);

    let (bytes, metadata) = result.expect("exact-size CSS must be valid");
    assert_eq!(bytes.len() as u64, MAX_CSS_FILE_BYTES);
    assert_eq!(metadata.len(), MAX_CSS_FILE_BYTES);
}
