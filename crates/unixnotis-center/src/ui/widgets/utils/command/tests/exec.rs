use std::io::{self, Cursor};
use std::path::Path;

use super::{read_to_end_limited, resolve_simple_program_from_root, MAX_CAPTURE_BYTES};

#[test]
fn read_to_end_limited_accepts_small_payloads() {
    let payload = b"ok".to_vec();
    let result = read_to_end_limited(Cursor::new(payload.clone())).expect("small payload");
    assert_eq!(result, payload);
}

#[test]
fn read_to_end_limited_rejects_oversized_payloads() {
    let payload = vec![0u8; MAX_CAPTURE_BYTES + 1];
    let err = read_to_end_limited(Cursor::new(payload)).expect_err("oversized payload");
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn resolve_simple_program_roots_relative_script_paths_in_config_dir() {
    let config_dir = Path::new("/tmp/demo/unixnotis");

    assert_eq!(
        resolve_simple_program_from_root(Some(config_dir), "scripts/demo-widget"),
        config_dir.join("scripts/demo-widget")
    );
}

#[test]
fn resolve_simple_program_blocks_parent_traversal_paths() {
    let config_dir = Path::new("/tmp/demo/unixnotis");

    assert_eq!(
        resolve_simple_program_from_root(Some(config_dir), "../outside-script"),
        config_dir.join(".unixnotis-blocked-command-path")
    );
}
