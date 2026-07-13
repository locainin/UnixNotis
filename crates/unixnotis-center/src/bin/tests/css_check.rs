use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

// Unique paths keep parallel parser tests from sharing filesystem state
fn unique_temp_path(label: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    path.push(format!("unixnotis-css-check-{pid}-{nanos}-{label}"));
    path
}

#[test]
fn parse_args_returns_none_for_empty() {
    let result = parse_args(std::iter::empty::<String>());
    assert!(result.is_none());
}

#[test]
fn parse_args_returns_paths_for_non_empty() {
    let result = parse_args(["a.css", "b.css"]).expect("paths should be present");
    assert_eq!(result, vec![PathBuf::from("a.css"), PathBuf::from("b.css")]);
}

#[test]
fn partition_existing_paths_splits_missing_and_existing() {
    let existing_path = unique_temp_path("existing.css");
    fs::write(&existing_path, "body {}").expect("temp file write should succeed");
    let missing_path = unique_temp_path("missing.css");

    let (existing, missing) =
        partition_existing_paths(vec![existing_path.clone(), missing_path.clone()]);

    assert_eq!(existing, vec![existing_path.clone()]);
    assert_eq!(missing, vec![missing_path]);

    fs::remove_file(&existing_path).expect("temp file cleanup should succeed");
}
