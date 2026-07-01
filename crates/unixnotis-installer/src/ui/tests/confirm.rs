use crate::app::Screen;
use crate::checks::{CheckItem, CheckState};
use crate::model::{ActionMode, ResetAction};

use super::test_support::{app_for_rendering, render_app};

#[test]
fn draw_confirm_reports_blocked_install_reason() {
    let mut app = app_for_rendering(Screen::Confirm(ActionMode::Install));
    app.checks.wayland = CheckItem {
        label: "Wayland",
        state: CheckState::Fail,
        detail: "missing".to_string(),
    };

    let screen = render_app(&app);

    // Blocking reasons need to appear before any action is allowed to run
    assert!(screen.contains("Confirm Install"));
    assert!(screen.contains("Blocked: Wayland session required"));
    assert!(!screen.contains("Reinstall will overwrite"));
}

#[test]
fn draw_confirm_reset_defaults_warns_about_backups() {
    let mut app = app_for_rendering(Screen::Confirm(ActionMode::Reset));
    app.reset_action = ResetAction::ResetDefaults;

    let screen = render_app(&app);

    // Reset confirmation should state overwrite and backup behavior together
    assert!(screen.contains("Confirm Reset config"));
    assert!(screen.contains("Reset overwrites config.toml"));
    assert!(screen.contains("Existing files are backed up"));
    assert!(screen.contains("Resetting to defaults"));
}

#[test]
fn draw_confirm_restore_backup_shows_selected_backup_path() {
    let mut app = app_for_rendering(Screen::Confirm(ActionMode::Reset));
    app.reset_action = ResetAction::RestoreBackup {
        path: "/tmp/Backup-2026-01-02".into(),
    };

    let screen = render_app(&app);

    // Restore confirmation must include the selected backup so the user can catch mistakes
    assert!(screen.contains("Restoring from backup"));
    assert!(screen.contains("Backup-2026-01-02"));
}
