use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::actions::{BuildAccelConfigStatus, BuildAccelDetection};
use crate::app::{App, BuildAccelState, ProgressState, Screen};
use crate::main_handlers::{
    handle_build_accel_key, handle_progress_key, handle_reset_menu_key, handle_restore_select_key,
    handle_welcome_key,
};
use crate::model::ActionMode;
use crate::ExitAction;

fn key(code: KeyCode) -> KeyEvent {
    // Tests build real crossterm key events so handler behavior matches the TUI path
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn vim_keys_move_welcome_menu_like_arrow_keys() {
    let _lock = crate::tests::env::test_env_lock();
    let mut app = App::new(None);

    // j/k should mirror Down/Up without changing menu bounds
    handle_welcome_key(&mut app, key(KeyCode::Char('j'))).expect("j should be handled");
    assert_eq!(app.menu_index, 1);

    handle_welcome_key(&mut app, key(KeyCode::Char('k'))).expect("k should be handled");
    assert_eq!(app.menu_index, 0);

    // Extra movement at the top should clamp instead of wrapping
    handle_welcome_key(&mut app, key(KeyCode::Char('k'))).expect("k should clamp at top");
    assert_eq!(app.menu_index, 0);
}

#[test]
fn welcome_menu_quit_key_exits_without_starting_action() {
    let _lock = crate::tests::env::test_env_lock();
    let mut app = App::new(None);

    let action = handle_welcome_key(&mut app, key(KeyCode::Char('q'))).expect("q should exit");

    // Quit should be an explicit exit action, not a silent screen transition
    assert!(matches!(action, Some(ExitAction::None)));
    assert_eq!(app.screen, Screen::Welcome);
}

#[test]
fn welcome_enter_opens_confirm_or_reset_submenu_for_selected_action() {
    let _lock = crate::tests::env::test_env_lock();
    let mut app = App::new(None);

    app.menu_index = 1;
    handle_welcome_key(&mut app, key(KeyCode::Enter)).expect("install enter");
    assert_eq!(app.screen, Screen::Confirm(ActionMode::Install));

    app.screen = Screen::Welcome;
    app.menu_index = 2;
    app.reset_menu_index = 2;
    handle_welcome_key(&mut app, key(KeyCode::Enter)).expect("reset enter");
    assert_eq!(app.screen, Screen::ResetMenu);
    assert_eq!(app.reset_menu_index, 0);
}

#[test]
fn vim_keys_move_reset_menu_like_arrow_keys() {
    let _lock = crate::tests::env::test_env_lock();
    let mut app = App::new(None);

    // Reset has a fixed three-entry menu, so j/k must stay within 0..=2
    handle_reset_menu_key(&mut app, key(KeyCode::Char('j'))).expect("j should be handled");
    handle_reset_menu_key(&mut app, key(KeyCode::Char('j'))).expect("j should be handled");
    handle_reset_menu_key(&mut app, key(KeyCode::Char('j'))).expect("j should clamp");
    assert_eq!(app.reset_menu_index, 2);

    handle_reset_menu_key(&mut app, key(KeyCode::Char('k'))).expect("k should be handled");
    assert_eq!(app.reset_menu_index, 1);
}

#[test]
fn reset_menu_escape_and_enter_select_expected_destinations() {
    let _lock = crate::tests::env::test_env_lock();
    let mut app = App::new(None);
    app.screen = Screen::ResetMenu;

    handle_reset_menu_key(&mut app, key(KeyCode::Esc)).expect("escape should return");
    assert_eq!(app.screen, Screen::Welcome);

    app.screen = Screen::ResetMenu;
    app.reset_menu_index = 0;
    handle_reset_menu_key(&mut app, key(KeyCode::Enter)).expect("defaults enter");
    assert_eq!(app.screen, Screen::Confirm(ActionMode::Reset));

    app.screen = Screen::ResetMenu;
    app.reset_menu_index = 1;
    handle_reset_menu_key(&mut app, key(KeyCode::Enter)).expect("restore enter");
    assert_eq!(app.screen, Screen::RestoreSelect);

    app.screen = Screen::ResetMenu;
    app.reset_menu_index = 2;
    handle_reset_menu_key(&mut app, key(KeyCode::Enter)).expect("cancel enter");
    assert_eq!(app.screen, Screen::Welcome);
}

#[test]
fn vim_keys_move_restore_selection_only_when_backups_exist() {
    let _lock = crate::tests::env::test_env_lock();
    let mut app = App::new(None);

    // Empty restore lists should not underflow or invent a selection
    handle_restore_select_key(&mut app, key(KeyCode::Char('j'))).expect("j should be handled");
    assert_eq!(app.restore_menu_index, 0);

    app.restore_backups = vec!["first".into(), "second".into()];
    handle_restore_select_key(&mut app, key(KeyCode::Char('j'))).expect("j should be handled");
    assert_eq!(app.restore_menu_index, 1);

    handle_restore_select_key(&mut app, key(KeyCode::Char('k'))).expect("k should be handled");
    assert_eq!(app.restore_menu_index, 0);
}

#[test]
fn restore_selection_escape_and_enter_only_confirm_existing_backup() {
    let _lock = crate::tests::env::test_env_lock();
    let mut app = App::new(None);
    app.screen = Screen::RestoreSelect;

    handle_restore_select_key(&mut app, key(KeyCode::Enter)).expect("empty enter");
    assert_eq!(app.screen, Screen::RestoreSelect);

    app.restore_backups = vec!["first".into(), "second".into()];
    app.restore_menu_index = 1;
    handle_restore_select_key(&mut app, key(KeyCode::Enter)).expect("backup enter");
    assert_eq!(app.screen, Screen::Confirm(ActionMode::Reset));

    app.screen = Screen::RestoreSelect;
    handle_restore_select_key(&mut app, key(KeyCode::Esc)).expect("escape should return");
    assert_eq!(app.screen, Screen::ResetMenu);
}

#[test]
fn progress_screen_ignores_keys_while_running_and_returns_after_completion() {
    let _lock = crate::tests::env::test_env_lock();
    let mut app = App::new(None);
    app.screen = Screen::Progress(ActionMode::Uninstall);
    app.progress_state = ProgressState::Running;

    let action = handle_progress_key(&mut app, key(KeyCode::Char('q'))).expect("running key");
    assert!(action.is_none());
    assert_eq!(app.screen, Screen::Progress(ActionMode::Uninstall));

    app.progress_state = ProgressState::Completed;
    app.progress_ready_at = None;
    handle_progress_key(&mut app, key(KeyCode::Enter)).expect("completed enter");
    assert_eq!(app.screen, Screen::Welcome);
    assert_eq!(app.progress_state, ProgressState::Idle);
}

#[test]
fn progress_screen_quit_and_escape_work_after_action_finishes() {
    let _lock = crate::tests::env::test_env_lock();
    let mut app = App::new(None);
    app.screen = Screen::Progress(ActionMode::Install);
    app.progress_state = ProgressState::Failed;
    app.progress_ready_at = None;

    let action = handle_progress_key(&mut app, key(KeyCode::Char('Q'))).expect("quit key");
    assert!(matches!(action, Some(ExitAction::None)));

    app.screen = Screen::Progress(ActionMode::Install);
    let action = handle_progress_key(&mut app, key(KeyCode::Esc)).expect("escape key");
    assert!(action.is_none());
    assert_eq!(app.screen, Screen::Welcome);
}

#[test]
fn vim_keys_move_build_accel_menu_like_arrow_keys() {
    let _lock = crate::tests::env::test_env_lock();
    let mut app = App::new(None);
    app.build_accel = Some(BuildAccelState {
        detection: BuildAccelDetection {
            sccache_installed: true,
            mold_installed: false,
            config_status: BuildAccelConfigStatus::Missing,
        },
        outcome: None,
    });

    // Build acceleration uses dynamic menu length, so j/k must respect that mode
    handle_build_accel_key(&mut app, key(KeyCode::Char('j'))).expect("j should be handled");
    handle_build_accel_key(&mut app, key(KeyCode::Char('j'))).expect("j should clamp");
    assert_eq!(app.build_accel_menu_index, 1);

    handle_build_accel_key(&mut app, key(KeyCode::Char('k'))).expect("k should be handled");
    assert_eq!(app.build_accel_menu_index, 0);
}
