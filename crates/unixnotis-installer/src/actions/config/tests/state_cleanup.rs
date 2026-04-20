use super::super::state::{
    cleanup_warning_message, format_with_state_env, remove_state_file, DirCleanupOutcome,
    DND_STATE_FILE,
};
use std::fs;
use std::path::PathBuf;
use unixnotis_core::util;

#[test]
fn resolve_state_dir_prefers_xdg_state_home() {
    // Ensures explicit XDG_STATE_HOME is used when provided.
    let Ok(home) = std::env::var("HOME") else {
        return;
    };
    if home.trim().is_empty() {
        return;
    }
    let xdg = PathBuf::from(&home).join(".state-test");
    let dir =
        util::resolve_state_dir_from_env(Some(xdg.to_string_lossy().as_ref()), Some(home.as_str()));
    assert_eq!(dir, Some(xdg));
}

#[test]
fn resolve_state_dir_falls_back_to_home() {
    // Ensures HOME/.local/state is used when XDG_STATE_HOME is empty.
    let Ok(home) = std::env::var("HOME") else {
        return;
    };
    if home.trim().is_empty() {
        return;
    }
    let dir = util::resolve_state_dir_from_env(Some("  "), Some(home.as_str()));
    assert_eq!(dir, Some(PathBuf::from(&home).join(".local").join("state")));
}

#[test]
fn remove_state_file_cleans_up_directory_when_empty() {
    // Confirms state.json removal cleans the directory when no other files exist.
    let root = PathBuf::from("target").join(format!(
        "unixnotis-installer-state-test-{}",
        std::process::id()
    ));
    let _ = fs::create_dir_all(&root);
    let state_path = root.join(DND_STATE_FILE);
    let _ = fs::write(&state_path, "{}");

    let outcome = remove_state_file(&root).expect("state removal should succeed");
    assert!(outcome.removed_file);
    assert!(!state_path.exists());
    assert!(outcome.removed_dir || !root.exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_state_file_keeps_directory_when_not_empty() {
    // Ensures unrelated files keep the state directory in place.
    let root = PathBuf::from("target").join(format!(
        "unixnotis-installer-state-nonempty-test-{}",
        std::process::id()
    ));
    let _ = fs::create_dir_all(&root);
    let state_path = root.join(DND_STATE_FILE);
    let other_path = root.join("extra.txt");
    let _ = fs::write(&state_path, "{}");
    let _ = fs::write(&other_path, "keep");

    let outcome = remove_state_file(&root).expect("state removal should succeed");
    assert!(outcome.removed_file);
    assert!(!state_path.exists());
    assert!(!outcome.removed_dir);
    assert!(root.exists());
    assert!(other_path.exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn cleanup_warning_message_flags_directory_inspection_failures() {
    // This covers the path where state.json is gone but read_dir fails afterward
    let root = PathBuf::from("target").join("unixnotis-installer-state-warning-test");
    let warning =
        cleanup_warning_message(&root, DirCleanupOutcome::InspectFailed).expect("warning expected");

    // The warning should explain which cleanup step failed
    assert!(warning.contains("failed to inspect state directory"));
    // Keep the file name in the message so the sequence is obvious in logs
    assert!(warning.contains(DND_STATE_FILE));
}

#[test]
fn cleanup_warning_message_flags_empty_directory_removal_failures() {
    // This covers the path where the dir looked empty but remove_dir still failed
    let root = PathBuf::from("target").join("unixnotis-installer-state-remove-warning-test");
    let warning =
        cleanup_warning_message(&root, DirCleanupOutcome::RemoveFailed).expect("warning expected");

    // The warning should make it clear that the leftover path is the empty state dir
    assert!(warning.contains("failed to remove empty state directory"));
    // Keep the file name in the message so the earlier successful delete is still visible
    assert!(warning.contains(DND_STATE_FILE));
}

#[test]
fn format_with_state_env_uses_xdg_state_home_prefix() {
    // Ensures state paths are rendered with $XDG_STATE_HOME when available.
    let key = "XDG_STATE_HOME";
    let original = std::env::var(key).ok();
    std::env::set_var(key, "state-root");

    let path = PathBuf::from("state-root")
        .join("unixnotis")
        .join(DND_STATE_FILE);
    let rendered = format_with_state_env(&path);
    assert!(rendered.starts_with("$XDG_STATE_HOME"));

    match original {
        Some(value) => std::env::set_var(key, value),
        None => std::env::remove_var(key),
    }
}
