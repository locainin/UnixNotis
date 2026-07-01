use std::collections::VecDeque;

use crate::app::{ProgressState, Screen};
use crate::model::{ActionMode, ResetAction};

use super::test_support::{app_for_rendering, render_app};

#[test]
fn draw_progress_renders_steps_logs_and_error_summary() {
    let mut app = app_for_rendering(Screen::Progress(ActionMode::Install));
    app.progress_state = ProgressState::Failed;

    let screen = render_app(&app);

    // The progress screen must keep both the short error and full logs visible
    assert!(screen.contains("Install - Failed"));
    assert!(screen.contains("Check existing install"));
    assert!(screen.contains("Enable user service"));
    assert!(screen.contains("cargo command failed"));
    assert!(screen.contains("second log"));
    assert!(!screen.contains("Enter = build acceleration options"));
}

#[test]
fn draw_progress_completed_install_points_to_build_acceleration() {
    let mut app = app_for_rendering(Screen::Progress(ActionMode::Install));
    app.progress_state = ProgressState::Completed;
    app.last_error = None;

    let screen = render_app(&app);

    // Successful install has a special next step before returning to the main menu
    assert!(screen.contains("Install - Completed"));
    assert!(screen.contains("Enter = build acceleration options"));
    assert!(!screen.contains("See logs for full output"));
}

#[test]
fn draw_progress_restore_backup_uses_restore_label() {
    let mut app = app_for_rendering(Screen::Progress(ActionMode::Reset));
    app.progress_state = ProgressState::Completed;
    app.reset_action = ResetAction::RestoreBackup {
        path: "/tmp/Backup-2026-01-02".into(),
    };

    let screen = render_app(&app);

    // Reset progress title must reflect restore mode so backup restores are not mislabeled
    assert!(screen.contains("Restore backup - Completed"));
    assert!(!screen.contains("Reset config - Completed"));
}

#[test]
fn draw_progress_running_state_uses_running_footer_without_error_summary() {
    let mut app = app_for_rendering(Screen::Progress(ActionMode::Uninstall));
    app.progress_state = ProgressState::Running;
    app.logs = VecDeque::from(["stopping service".to_string()]);

    let screen = render_app(&app);

    // Running actions should not expose stale error text from an earlier failed action
    assert!(screen.contains("Uninstall - In progress"));
    assert!(screen.contains("Running..."));
    assert!(!screen.contains("Error:"));
}
