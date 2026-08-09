//! Top-level action setup and worker launch

use anyhow::Result;
use std::sync::mpsc;
use std::thread;

use crate::actions::{build_plan, check_install_state, steps_from_plan, InstallerLock, StepKind};
use crate::app::events::UiMessage;
use crate::app::workflow::worker::{action_requires_install_state, run_action_worker};
use crate::app::{App, ProgressState, Screen};
use crate::model::ActionMode;
use crate::paths::InstallPaths;

pub fn start_action<F>(
    app: &mut App,
    draw_action: F,
    ui_tx: &mpsc::SyncSender<UiMessage>,
    mode: ActionMode,
) -> Result<()>
where
    F: FnOnce(&App) -> Result<()>,
{
    // Resolve paths once so every step in this action uses the same install target
    let paths = InstallPaths::discover_with_service_manager(app.service_manager)?;
    // The retained descriptor serializes every mutating step across installer processes
    let installer_lock = InstallerLock::acquire_for_session()?;
    // Install state is only needed for install decisions like service start mode
    let install_state = if action_requires_install_state(mode) {
        Some(check_install_state(&paths))
    } else {
        None
    };

    let (plan, restore_backup) = match mode {
        ActionMode::Reset => match &app.reset_action {
            // Default reset uses the normal reset plan
            crate::model::ResetAction::ResetDefaults => (build_plan(mode), None),
            crate::model::ResetAction::RestoreBackup { path } => {
                // Restore runs only the restore step and carries the chosen backup path
                (vec![StepKind::RestoreConfig], Some(path.clone()))
            }
        },
        _ => (build_plan(mode), None),
    };

    // Reset visible progress state before the worker starts sending events
    app.steps = steps_from_plan(&plan);
    app.logs.clear();
    app.last_error = None;
    app.progress_state = ProgressState::Running;
    app.progress_ready_at = None;
    app.screen = Screen::Progress(mode);

    draw_action(app)?;

    let ui_tx = ui_tx.clone();
    thread::spawn(move || {
        let _installer_lock = installer_lock;
        run_action_worker(
            &plan,
            mode,
            &paths,
            install_state.as_ref(),
            restore_backup.as_deref(),
            &ui_tx,
        );
    });

    Ok(())
}
