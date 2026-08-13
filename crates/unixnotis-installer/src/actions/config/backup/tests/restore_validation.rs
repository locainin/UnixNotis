use std::fs;
use std::path::PathBuf;

use super::{
    backup_entry_exists, build_restore_plan, read_backup_file_bounded, reject_duplicate_targets,
    validate_backup_directory_name, RestoreFile, MAX_RESTORE_FILE_BYTES,
};

#[test]
fn restore_file_budget_keeps_its_declared_byte_domain() {
    assert_eq!(MAX_RESTORE_FILE_BYTES, 16_777_216);
}

#[test]
fn restore_source_reader_accepts_exact_limit_and_rejects_one_extra_byte() {
    let root = crate::test_support::fs::unique_temp_path("restore-reader-boundary");
    fs::create_dir_all(&root).expect("create restore reader fixture");
    let source = root.join("source");
    fs::write(&source, vec![b'x'; 4_096]).expect("write exact-limit source");

    assert_eq!(
        read_backup_file_bounded(&source, 4_096)
            .expect("exact restore source limit")
            .len(),
        4_096
    );
    fs::write(&source, vec![b'x'; 4_097]).expect("write oversized source");
    assert!(read_backup_file_bounded(&source, 4_096).is_err());
    fs::remove_dir_all(root).expect("remove restore reader fixture");
}

#[test]
fn backup_directory_validation_rejects_unrecognized_names() {
    assert!(validate_backup_directory_name(&PathBuf::from("Backup-valid")).is_ok());
    assert!(validate_backup_directory_name(&PathBuf::from("unrecognized")).is_err());
}

#[test]
fn duplicate_restore_targets_are_rejected_before_commit() {
    let target = std::env::temp_dir().join("unixnotis-duplicate-restore-target");
    let files = [
        RestoreFile {
            label: "config.toml".to_string(),
            target: target.clone(),
            mode: 0o644,
            contents: Vec::new(),
        },
        RestoreFile {
            label: "base.css".to_string(),
            target,
            mode: 0o644,
            contents: Vec::new(),
        },
    ];

    assert!(reject_duplicate_targets(&files).is_err());
}

#[test]
fn backup_entry_probe_propagates_lookup_errors() {
    let root = crate::test_support::fs::unique_temp_path("restore-entry-probe-error");
    fs::create_dir_all(&root).expect("create restore probe fixture");
    let regular_parent = root.join("regular-parent");
    fs::write(&regular_parent, "not a directory").expect("write invalid parent");

    assert!(
        backup_entry_exists(&regular_parent.join("target")).is_err(),
        "lookup errors must not become absent backup entries"
    );
    fs::remove_dir_all(root).expect("remove restore probe fixture");
}

#[test]
fn restore_plan_publishes_supporting_payloads_before_config() {
    let root = crate::test_support::fs::unique_temp_path("restore-config-last");
    let config_dir = root.join("unixnotis");
    let backup_dir = config_dir.join("Backup-config-last");
    fs::create_dir_all(&backup_dir).expect("create restore plan fixture");
    fs::write(
        backup_dir.join("config.toml"),
        crate::test_support::current_config_text(""),
    )
    .expect("write backup config");
    fs::write(backup_dir.join("base.css"), "restored base\n")
        .expect("write supporting theme payload");

    let plan = build_restore_plan(&backup_dir, &config_dir).expect("build restore plan");
    let labels = plan
        .files
        .iter()
        .map(|file| file.label.as_str())
        .collect::<Vec<_>>();
    let base_index = labels
        .iter()
        .position(|label| *label == "base.css")
        .expect("supporting base theme in plan");
    let config_index = labels
        .iter()
        .position(|label| *label == "config.toml")
        .expect("config in plan");

    assert_eq!(labels.last(), Some(&"config.toml"));
    assert!(
        base_index < config_index,
        "supporting theme payload must publish before its config reference"
    );
    fs::remove_dir_all(root).expect("remove restore plan fixture");
}
