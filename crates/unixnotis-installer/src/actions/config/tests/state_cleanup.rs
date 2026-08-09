use super::super::state::{
    cleanup_warning_message, format_with_state_env, remove_state, remove_state_file,
    DirCleanupOutcome, DND_STATE_FILE,
};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};

use crate::actions::ActionContext;
use crate::app::events::UiMessage;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;
use crate::test_support::env::EnvGuard;
use unixnotis_core::util;

#[test]
fn resolve_state_dir_prefers_xdg_state_home() {
    // Ensures explicit XDG_STATE_HOME is used when provided
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
    // Ensures HOME/.local/state is used when XDG_STATE_HOME is empty
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
    // Confirms state.json removal cleans the directory when no other files exist
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
    // Ensures unrelated files keep the state directory in place
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
    assert!(outcome.cleanup_warning.is_none());
    assert!(root.exists());
    assert!(other_path.exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_state_file_returns_unchanged_when_state_file_is_missing() {
    let root = std::env::temp_dir().join(format!(
        "unixnotis-installer-state-missing-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("make empty state directory");

    let outcome = remove_state_file(&root).expect("missing state should be a safe no-op");

    assert!(!outcome.removed_file);
    assert!(!outcome.removed_dir);
    assert!(outcome.cleanup_warning.is_none());
    assert!(root.exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_state_file_propagates_non_missing_filesystem_errors() {
    let root = std::env::temp_dir().join(format!(
        "unixnotis-installer-state-error-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.parent().expect("temporary root parent"))
        .expect("make temporary parent");
    // A regular state root makes state.json lookup fail with NotADirectory
    fs::write(&root, "not a directory").expect("write blocking state root");

    let error = remove_state_file(&root).expect_err("non-missing error must be preserved");

    assert_ne!(error.kind(), std::io::ErrorKind::NotFound);
    assert!(root.is_file());
    let _ = fs::remove_file(&root);
}

#[test]
fn remove_state_file_rejects_symlink_without_touching_its_target() {
    let root = crate::test_support::fs::unique_temp_path("remove-state-symlink");
    let state_root = root.join("unixnotis");
    let state_file = state_root.join(DND_STATE_FILE);
    let protected = root.join("protected");
    fs::create_dir_all(&state_root).expect("create state directory");
    fs::write(&protected, "protected").expect("write protected file");
    symlink(&protected, &state_file).expect("create state link");

    remove_state_file(&state_root).expect_err("state link should be rejected");

    assert_eq!(
        fs::read_to_string(&protected).expect("read protected file"),
        "protected"
    );
    assert!(fs::symlink_metadata(&state_file)
        .expect("state link remains")
        .file_type()
        .is_symlink());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn remove_state_uses_xdg_state_home_and_deletes_persisted_state() {
    let _lock = crate::test_support::env::test_env_lock();
    let state_home = std::env::temp_dir().join(format!(
        "unixnotis-installer-remove-state-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&state_home);
    let state_root = state_home.join("unixnotis");
    let state_file = state_root.join(DND_STATE_FILE);
    fs::create_dir_all(&state_root).expect("make state directory");
    fs::write(&state_file, "{\"dnd\":true}").expect("seed persisted state");
    let _xdg_state_home = EnvGuard::set("XDG_STATE_HOME", &state_home);
    let paths = InstallPaths {
        repo_root: state_home.join("repo"),
        bin_dir: state_home.join("bin"),
        service: ServiceManager::systemd_user(state_home.join("service")),
    };
    let (log_tx, log_rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = ActionContext {
        paths: &paths,
        install_state: None,
        log_tx,
        action_mode: ActionMode::Uninstall,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    remove_state(&mut ctx).expect("top-level state cleanup should succeed");

    assert!(!state_file.exists());
    assert!(log_rx.try_iter().any(|message| matches!(
        message,
        UiMessage::Worker(crate::app::events::WorkerEvent::LogLine(line))
            if line.contains("Removed persisted state file")
    )));
    let _ = fs::remove_dir_all(&state_home);
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
    // Ensures state paths are rendered with $XDG_STATE_HOME when available
    let _lock = crate::test_support::env::test_env_lock();
    let _xdg_state_home = EnvGuard::set("XDG_STATE_HOME", "state-root");

    let path = PathBuf::from("state-root")
        .join("unixnotis")
        .join(DND_STATE_FILE);
    let rendered = format_with_state_env(&path);
    assert!(rendered.starts_with("$XDG_STATE_HOME"));
}
