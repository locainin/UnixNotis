use std::path::PathBuf;

use super::super::file_path_from_hint;

#[test]
fn file_path_from_hint_accepts_absolute_paths_and_local_file_uris_only() {
    assert_eq!(
        file_path_from_hint("/tmp/unixnotis/icon.png"),
        Some(PathBuf::from("/tmp/unixnotis/icon.png"))
    );
    assert_eq!(
        file_path_from_hint("file:///tmp/unixnotis/icon%20name.png"),
        Some(PathBuf::from("/tmp/unixnotis/icon name.png"))
    );
    assert!(file_path_from_hint("https://example.com/icon.png").is_none());
    assert!(file_path_from_hint("relative/icon.png").is_none());
}
