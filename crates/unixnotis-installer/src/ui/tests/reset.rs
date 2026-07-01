use crate::app::Screen;
use crate::model::ResetAction;
use ratatui::style::{Color, Modifier};

use super::test_support::{app_for_rendering, render_app, render_widget_buffer, style_for_text};

#[test]
fn draw_reset_menu_renders_current_summary_and_choices() {
    let app = app_for_rendering(Screen::ResetMenu);

    let screen = render_app(&app);

    // Reset content is intentionally specific because the action can overwrite files
    assert!(screen.contains("Reset actions"));
    assert!(screen.contains("Reset to defaults"));
    assert!(screen.contains("Restore backup"));
    assert!(screen.contains("Overwrites config.toml"));
}

#[test]
fn draw_restore_select_renders_available_backups() {
    let app = app_for_rendering(Screen::RestoreSelect);

    let screen = render_app(&app);

    // Backup paths are the only way to verify the selected restore target before confirm
    assert!(screen.contains("Available backups"));
    assert!(screen.contains("Backup-2026-01-01"));
    assert!(screen.contains("Backup-2026-01-02"));
}

#[test]
fn draw_restore_select_handles_empty_backup_list() {
    let mut app = app_for_rendering(Screen::RestoreSelect);
    app.restore_backups.clear();

    let screen = render_app(&app);

    // Empty restore state should be explicit, not a blank list
    assert!(screen.contains("No backups found"));
    assert!(!screen.contains("Backup-2026-01-01"));
}

#[test]
fn reset_summary_tracks_selected_action() {
    let mut app = app_for_rendering(Screen::ResetMenu);
    app.reset_menu_index = 1;

    let lines = super::reset::build_reset_summary(&app);
    let rendered = format!("{lines:?}");

    // The summary must follow the selected row, not always default to destructive reset text
    assert!(rendered.contains("Restore backup"));
    assert!(rendered.contains("selected backup"));
    assert!(!rendered.contains("Overwrites config.toml"));
}

#[test]
fn reset_menu_highlights_selected_action_only() {
    let mut app = app_for_rendering(Screen::ResetMenu);
    app.reset_menu_index = 1;

    let list = super::reset::render_reset_menu(&app, 60);
    let buffer = render_widget_buffer(list, 60, 4);
    let selected = style_for_text(&buffer, "Restore backup");
    let unselected = style_for_text(&buffer, "Reset to defaults");

    // The selected row is style-only, so inspect the backing buffer directly
    assert_eq!(selected.fg, Some(Color::Black));
    assert_eq!(selected.bg, Some(Color::Cyan));
    assert!(selected.add_modifier.contains(Modifier::BOLD));
    assert_ne!(unselected.bg, Some(Color::Cyan));
}

#[test]
fn restore_menu_highlights_selected_backup_only() {
    let app = app_for_rendering(Screen::RestoreSelect);

    let list = super::reset::render_restore_menu(&app, 80);
    let buffer = render_widget_buffer(list, 80, 4);
    let unselected = style_for_text(&buffer, "Backup-2026-01-01");
    let selected = style_for_text(&buffer, "Backup-2026-01-02");

    // Backup selection must follow restore_menu_index, not the first available backup
    assert_eq!(selected.fg, Some(Color::Black));
    assert_eq!(selected.bg, Some(Color::Cyan));
    assert!(selected.add_modifier.contains(Modifier::BOLD));
    assert_ne!(unselected.bg, Some(Color::Cyan));
}

#[test]
fn reset_summary_returns_safe_menu_text_for_return_row() {
    let mut app = app_for_rendering(Screen::ResetMenu);
    app.reset_menu_index = 2;

    let lines = super::reset::build_reset_summary(&app);
    let rendered = format!("{lines:?}");

    // The return row should not carry destructive reset or restore language
    assert!(rendered.contains("Return to menu"));
    assert!(rendered.contains("without making changes"));
    assert!(!rendered.contains("Overwrites config.toml"));
}

#[test]
fn describe_reset_action_distinguishes_defaults_from_restore_target() {
    let mut app = app_for_rendering(Screen::ResetMenu);
    app.reset_action = ResetAction::ResetDefaults;
    assert_eq!(
        super::reset::describe_reset_action(&app),
        "Resetting to defaults will overwrite config.toml and theme files."
    );

    app.reset_action = ResetAction::RestoreBackup {
        path: "/tmp/Backup-2026-01-02".into(),
    };
    let description = super::reset::describe_reset_action(&app);
    assert!(description.contains("Restoring from backup"));
    assert!(description.contains("Backup-2026-01-02"));
}
