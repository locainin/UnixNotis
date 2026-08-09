use std::sync::mpsc;

use super::super::controller::start_action;
use super::super::worker::action_requires_install_state;
use crate::app::events::{UiMessage, WorkerEvent};
use crate::app::{App, Screen};
use crate::model::ActionMode;

#[test]
fn start_action_prepares_and_draws_the_test_workflow_before_worker_completion() {
    let _lock = crate::test_support::env::test_env_lock();
    let runtime = crate::test_support::fs::unique_temp_path("start-action-runtime");
    std::fs::create_dir_all(&runtime).expect("create action runtime directory");
    let _runtime_env = crate::test_support::env::EnvGuard::set("XDG_RUNTIME_DIR", &runtime);
    let mut app = App::new(None);
    let mut draws = 0_u8;
    let (tx, rx) = mpsc::sync_channel(8);

    start_action(
        &mut app,
        |_| {
            draws = draws.saturating_add(1);
            Ok(())
        },
        &tx,
        ActionMode::Test,
    )
    .expect("start empty test action");

    assert_eq!(draws, 1);
    assert_eq!(app.screen, Screen::Progress(ActionMode::Test));
    assert_eq!(app.progress_state, crate::app::ProgressState::Running);
    assert!(matches!(
        rx.recv_timeout(std::time::Duration::from_secs(1))
            .expect("worker completion event"),
        UiMessage::Worker(WorkerEvent::Finished)
    ));
    std::fs::remove_dir_all(runtime).expect("remove action runtime directory");
}

#[test]
fn only_install_actions_capture_the_pre_action_install_state() {
    assert!(action_requires_install_state(ActionMode::Install));
    assert!(!action_requires_install_state(ActionMode::Test));
    assert!(!action_requires_install_state(ActionMode::Reset));
    assert!(!action_requires_install_state(ActionMode::Uninstall));
}
