use std::path::PathBuf;

use crate::model::{ActionMode, ActionStep, ResetAction, StepStatus};

#[test]
fn action_mode_labels_match_user_facing_menu_text() {
    // Labels are rendered in menus and progress titles, so wording changes should be intentional
    assert_eq!(ActionMode::Test.label(), "Trial run");
    assert_eq!(ActionMode::Install.label(), "Install");
    assert_eq!(ActionMode::Uninstall.label(), "Uninstall");
    assert_eq!(ActionMode::Reset.label(), "Reset config");
}

#[test]
fn reset_action_restore_backup_keeps_selected_path() {
    let backup_path = PathBuf::from("/tmp/unixnotis-backup");
    let action = ResetAction::RestoreBackup {
        path: backup_path.clone(),
    };

    // Restore state must keep the exact backup path selected in the UI
    assert_eq!(action, ResetAction::RestoreBackup { path: backup_path });
}

#[test]
fn action_step_tracks_name_and_status_without_side_effects() {
    let step = ActionStep {
        name: "Install service",
        status: StepStatus::Running,
    };

    // ActionStep stays a plain data object so workers and UI rendering can share it cheaply
    assert_eq!(step.name, "Install service");
    assert_eq!(step.status, StepStatus::Running);
}
