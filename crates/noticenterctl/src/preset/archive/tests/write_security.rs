use super::super::write::{create_temp_bundle_file, temp_bundle_path};
use super::support::TempDirGuard;
use std::fs;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::symlink;

#[cfg(unix)]
#[test]
fn create_temp_bundle_file_rejects_existing_symlink_without_touching_target() {
    let root = TempDirGuard::new("temp-symlink-refusal");
    let target = root.path.join("target");
    let temp_path = root.path.join(".bundle.unixnotis.fixed.tmp");
    fs::write(&target, "keep me").expect("write protected target");
    symlink(&target, &temp_path).expect("create temp symlink");

    let error = create_temp_bundle_file(&temp_path).expect_err("temp symlink must be refused");

    assert!(error.to_string().contains("create temp preset bundle"));
    assert_eq!(
        fs::read_to_string(&target).expect("read protected target"),
        "keep me"
    );
    assert!(Path::new(&temp_path).exists());
}

#[test]
fn create_temp_bundle_file_rejects_existing_regular_file() {
    let root = TempDirGuard::new("temp-regular-refusal");
    let temp_path = root.path.join(".bundle.unixnotis.fixed.tmp");
    fs::write(&temp_path, "existing").expect("write existing temp");

    let error =
        create_temp_bundle_file(&temp_path).expect_err("existing temp file must be refused");

    assert!(error.to_string().contains("create temp preset bundle"));
    assert_eq!(
        fs::read_to_string(&temp_path).expect("read existing temp"),
        "existing"
    );
}

#[test]
fn temp_bundle_path_uses_safe_fallback_for_empty_output_path() {
    let temp_path = temp_bundle_path(Path::new(""));

    let name = temp_path
        .file_name()
        .and_then(|value| value.to_str())
        .expect("temp path should have a file name");
    assert!(name.starts_with(".preset.unixnotis."));
    assert_eq!(
        temp_path.extension().and_then(|value| value.to_str()),
        Some("tmp")
    );
}

#[test]
fn temp_bundle_path_includes_non_empty_output_file_name() {
    let temp_path = temp_bundle_path(Path::new("custom.unixnotis"));

    let name = temp_path
        .file_name()
        .and_then(|value| value.to_str())
        .expect("temp path should have a file name");
    assert!(name.starts_with(".custom.unixnotis."));
    assert_eq!(
        temp_path.extension().and_then(|value| value.to_str()),
        Some("tmp")
    );
}
