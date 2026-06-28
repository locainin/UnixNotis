use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{
    handle_build_accel_key, handle_reset_menu_key, handle_restore_select_key, handle_welcome_key,
};
use crate::actions::{BuildAccelConfigStatus, BuildAccelDetection};
use crate::app::{App, BuildAccelState};

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
