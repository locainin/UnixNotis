use std::path::PathBuf;

use super::super::{classify_file_url, FileUrlClassification};

#[test]
fn file_url_classifier_accepts_local_forms_and_decodes_paths() {
    for value in [
        "file:///config/image%20one.png",
        "FILE://localhost/config/image%20one.png",
        "file:/config/image%20one.png",
    ] {
        assert_eq!(
            classify_file_url(value),
            FileUrlClassification::Local(PathBuf::from("/config/image one.png"))
        );
    }
}

#[test]
fn file_url_classifier_distinguishes_remote_malformed_and_unrelated_values() {
    assert_eq!(
        classify_file_url("file://remote.invalid/config/image.png"),
        FileUrlClassification::NonLocalAuthority
    );
    for value in [
        "file:///config/bad%ZZ.png",
        "file:///config/image.png?changed=1",
        "file:///config/image.png#fragment",
        "file:///config/nul%00byte.png",
    ] {
        assert_eq!(classify_file_url(value), FileUrlClassification::Malformed);
    }
    assert_eq!(
        classify_file_url("assets/image.png"),
        FileUrlClassification::NotFileUrl
    );
}
