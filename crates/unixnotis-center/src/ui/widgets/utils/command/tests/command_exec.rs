use std::io::{self, Cursor};
use std::path::Path;

use super::{
    build_command, read_to_end_limited, resolve_simple_program_from_root, MAX_CAPTURE_BYTES,
};

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
fn resolve_simple_program_uses_supplied_config_dir_for_relative_scripts() {
    let config_dir = Path::new("/tmp/unixnotis-custom-config-root");

    assert_eq!(
        resolve_simple_program_from_root(Some(config_dir), "scripts/unixnotis-blue-light-state"),
        config_dir.join("scripts/unixnotis-blue-light-state")
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

#[test]
fn shell_commands_inherit_process_working_directory() {
    let command = build_command(". ./lib/common.sh");

    // No implicit config-root directory exists, so relative shell operands are not portable
    assert_eq!(command.get_current_dir(), None);
}
