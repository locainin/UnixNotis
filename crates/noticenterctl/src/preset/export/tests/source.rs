use std::fs;
use std::path::{Path, PathBuf};

use super::super::source::ExportSourceSnapshot;
use super::super::tests::support::TempDirGuard;
use crate::preset::config_root::SecureFileCapture;

#[test]
fn config_capture_keeps_the_bytes_that_were_validated() {
    let root = TempDirGuard::new("config-source-snapshot");
    root.write("config.toml", "[theme]\nbase_css = \"first.css\"\n");
    let snapshot = ExportSourceSnapshot::capture(&root.path).expect("capture config source");

    fs::write(
        root.path.join("config.toml"),
        "[theme]\nbase_css = \"replaced.css\"\n",
    )
    .expect("replace live config path");

    let captured = snapshot
        .captures()
        .get(Path::new("config.toml"))
        .expect("captured config");
    assert_eq!(captured.contents, snapshot.config_bytes());
    assert!(String::from_utf8_lossy(&captured.contents).contains("first.css"));
    assert_eq!(snapshot.config().theme.base_css, PathBuf::from("first.css"));
}

#[test]
fn active_file_capture_survives_a_later_path_replacement() {
    let root = TempDirGuard::new("active-source-snapshot");
    root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    root.write("base.css", ".panel { color: blue; }");
    let mut snapshot = ExportSourceSnapshot::capture(&root.path).expect("capture config source");
    let stylesheet = root.path.join("base.css");

    let relative = snapshot
        .capture_active_files(&root.path, std::slice::from_ref(&stylesheet))
        .expect("capture active stylesheet");
    fs::write(&stylesheet, ".panel { color: red; }").expect("replace live stylesheet");

    assert_eq!(relative, vec![PathBuf::from("base.css")]);
    assert_eq!(
        snapshot
            .captures()
            .get(Path::new("base.css"))
            .expect("captured stylesheet")
            .contents,
        b".panel { color: blue; }"
    );
}

#[cfg(unix)]
#[test]
fn config_capture_rejects_a_symlink_instead_of_following_it() {
    let root = TempDirGuard::new("symlink-config-source");
    let outside = root.path.with_file_name("outside-config-source.toml");
    fs::write(&outside, "[theme]\nbase_css = \"outside.css\"\n").expect("write outside config");
    std::os::unix::fs::symlink(&outside, root.path.join("config.toml"))
        .expect("create config symlink");

    let error = ExportSourceSnapshot::capture(&root.path)
        .err()
        .expect("symlink config must be rejected");

    assert!(error.to_string().contains("securely capture config.toml"));
    fs::remove_file(outside).expect("remove outside config");
}

#[cfg(unix)]
#[test]
fn active_file_capture_propagates_metadata_errors_other_than_missing_files() {
    let root = TempDirGuard::new("active-source-metadata-error");
    root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    let mut snapshot = ExportSourceSnapshot::capture(&root.path).expect("capture config source");
    // Linux rejects one component beyond NAME_MAX before user permissions affect the result
    let invalid_path = root.path.join("x".repeat(300));

    let error = snapshot
        .capture_active_files(&root.path, &[invalid_path])
        .expect_err("path errors must not look like missing optional files");

    assert!(error.to_string().contains("inspect active file"));
}

#[test]
fn source_snapshot_rejects_a_replaced_live_config_root() {
    let root = TempDirGuard::new("source-root-replacement");
    root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    let snapshot = ExportSourceSnapshot::capture(&root.path).expect("capture config source");
    let moved = root.path.with_extension("moved");
    let _ = fs::remove_dir_all(&moved);
    fs::rename(&root.path, &moved).expect("move captured config root");
    fs::create_dir(&root.path).expect("create replacement config root");

    let error = snapshot
        .ensure_live_root(&root.path)
        .expect_err("replaced config root must invalidate the source snapshot");

    assert!(error.to_string().contains("config directory changed"));
    fs::remove_dir_all(&root.path).expect("remove replacement root");
    fs::rename(&moved, &root.path).expect("restore captured config root");
}

#[test]
fn extending_source_snapshot_retains_dependency_bytes_and_mode() {
    let root = TempDirGuard::new("source-extend-captures");
    root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    let mut snapshot = ExportSourceSnapshot::capture(&root.path).expect("capture config source");
    let relative = PathBuf::from("scripts/helper.sh");
    let expected_contents = b"#!/bin/sh\n".to_vec();
    let expected_mode = 0o755;

    snapshot.extend_captures(std::collections::BTreeMap::from([(
        relative.clone(),
        SecureFileCapture {
            contents: expected_contents.clone(),
            mode: expected_mode,
        },
    )]));

    let captured = snapshot
        .captures()
        .get(&relative)
        .expect("dependency capture should be retained");
    assert_eq!(captured.contents, expected_contents);
    assert_eq!(captured.mode, expected_mode);
}
