//! Build acceleration prompt state and repository-local setup

use crate::actions::{
    detect_build_accel, detect_build_accel_without_repo, write_build_accel_config,
    BuildAccelOutcome,
};
use crate::app::{App, BuildAccelMenuMode, BuildAccelState};
use crate::paths::InstallPaths;

pub fn prepare_build_accel_prompt(app: &mut App) {
    // Snapshot detection so the prompt remains stable while the user decides
    let detection = match InstallPaths::discover_with_service_manager(app.service_manager) {
        Ok(paths) => detect_build_accel(&paths.repo_root),
        Err(err) => detect_build_accel_without_repo(err.to_string()),
    };
    app.build_accel = Some(BuildAccelState {
        detection,
        outcome: None,
    });
    app.build_accel_menu_index = 0;
}

fn apply_build_accel_setup(app: &mut App) {
    // Writes per-repository Cargo config only when explicitly requested
    let Some(state) = app.build_accel.as_mut() else {
        return;
    };
    let paths = match InstallPaths::discover_with_service_manager(app.service_manager) {
        Ok(paths) => paths,
        Err(err) => {
            state.outcome = Some(BuildAccelOutcome::Failed(err.to_string()));
            return;
        }
    };
    let outcome = write_build_accel_config(&paths.repo_root, &state.detection);
    state.outcome = Some(outcome);
    // Keep selection on the only available action once a result is shown
    app.build_accel_menu_index = 0;
    // Refresh detection so config state is reflected in the prompt immediately
    state.detection = detect_build_accel(&paths.repo_root);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildAccelEnterAction {
    ReturnToMenu,
    ApplySetup,
}

pub const fn build_accel_enter_action(
    mode: BuildAccelMenuMode,
    selected_index: usize,
) -> BuildAccelEnterAction {
    // One pure mapping keeps menu order separate from filesystem side effects
    match mode {
        BuildAccelMenuMode::EnableOrSkip if selected_index == 0 => {
            BuildAccelEnterAction::ApplySetup
        }
        BuildAccelMenuMode::Reinstall if selected_index != 0 => BuildAccelEnterAction::ApplySetup,
        BuildAccelMenuMode::ReturnOnly
        | BuildAccelMenuMode::EnableOrSkip
        | BuildAccelMenuMode::Reinstall => BuildAccelEnterAction::ReturnToMenu,
    }
}

pub fn handle_build_accel_enter(app: &mut App) {
    let action = build_accel_enter_action(app.build_accel_menu_mode(), app.build_accel_menu_index);
    match action {
        BuildAccelEnterAction::ReturnToMenu => super::reset_to_menu(app),
        BuildAccelEnterAction::ApplySetup => apply_build_accel_setup(app),
    }
}
