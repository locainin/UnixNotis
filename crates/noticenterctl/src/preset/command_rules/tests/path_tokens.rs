use super::super::tokens::{is_host_specific_path_token, looks_like_path_token};

#[test]
fn path_token_detection_covers_every_supported_relative_form() {
    for token in ["~/tool", "./tool", "../tool", "dir/tool", "/tool"] {
        assert!(
            looks_like_path_token(token),
            "path form not detected: {token}"
        );
    }
    for token in ["", "tool", "tool-name", ".", "..", "~"] {
        assert!(
            !looks_like_path_token(token),
            "plain command was treated as a path: {token}"
        );
    }
}

#[test]
fn host_specific_path_detection_excludes_portable_relative_paths() {
    assert!(is_host_specific_path_token("/usr/bin/tool"));
    for token in ["tool", "./tool", "../tool", "dir/tool", "~", "~/bin/tool"] {
        assert!(
            !is_host_specific_path_token(token),
            "portable path was treated as host-specific: {token}"
        );
    }
}
