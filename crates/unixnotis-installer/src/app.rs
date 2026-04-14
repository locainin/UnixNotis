//! UI state and event handling for the installer TUI.

use crate::actions::{check_install_state, InstallState};
use crate::actions::{BuildAccelConfigStatus, BuildAccelDetection, BuildAccelOutcome};
use crate::checks::Checks;
use crate::detect::Detection;
use crate::model::{ActionMode, ActionStep, ResetAction};
use crate::paths::InstallPaths;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgressState {
    // No action is running.
    Idle,
    // Action is running.
    Running,
    // Action finished successfully.
    Completed,
    // Action failed.
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Screen {
    // Landing screen with status and menu.
    Welcome,
    // Confirmation screen before execution.
    Confirm(ActionMode),
    // Reset submenu for default vs restore.
    ResetMenu,
    // Backup selection screen for restore.
    RestoreSelect,
    // Progress screen for running actions.
    Progress(ActionMode),
    // Optional build-acceleration prompt after install.
    BuildAccel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuItem {
    // Select an action mode.
    Action(ActionMode),
    // Exit the application.
    Quit,
}

pub struct App {
    // Results of environment/system checks.
    pub checks: Checks,

    // Detection of existing daemons/services and ownership state.
    pub detection: Detection,

    // Selected menu index.
    pub menu_index: usize,

    // Current screen.
    pub screen: Screen,

    // Log lines for UI display.
    pub logs: VecDeque<String>,

    // Steps for the active action.
    pub steps: Vec<ActionStep>,

    // Progress state for the active action.
    pub progress_state: ProgressState,

    // Last error message for failure display.
    pub last_error: Option<String>,

    // Cached install state for dynamic menu labeling.
    pub install_state: Option<InstallState>,

    // Earliest time the progress screen can accept navigation input.
    pub progress_ready_at: Option<Instant>,

    // Optional build-acceleration prompt data.
    pub build_accel: Option<BuildAccelState>,

    // Selected option index for the build-acceleration prompt.
    pub build_accel_menu_index: usize,

    // Reset submenu selection index (defaults vs restore).
    pub reset_menu_index: usize,

    // Selected reset action (defaults or a specific backup restore).
    pub reset_action: ResetAction,

    // Cached list of backup directories for the restore flow.
    pub restore_backups: Vec<PathBuf>,
    // Selected backup index when restoring.
    pub restore_menu_index: usize,
}

#[derive(Clone, Debug)]
pub struct BuildAccelState {
    pub detection: BuildAccelDetection,
    pub outcome: Option<BuildAccelOutcome>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildAccelMenuMode {
    ReturnOnly,
    EnableOrSkip,
    Reinstall,
}

impl App {
    pub fn new() -> Self {
        // Initialize with current system state.
        let (checks, detection, install_state) = Self::load_state();

        Self {
            checks,
            detection,
            menu_index: 0,
            screen: Screen::Welcome,
            logs: VecDeque::new(),
            steps: Vec::new(),
            progress_state: ProgressState::Idle,
            last_error: None,
            install_state,
            progress_ready_at: None,
            build_accel: None,
            build_accel_menu_index: 0,
            reset_menu_index: 0,
            reset_action: ResetAction::ResetDefaults,
            restore_backups: Vec::new(),
            restore_menu_index: 0,
        }
    }

    pub fn menu_items() -> [MenuItem; 5] {
        [
            MenuItem::Action(ActionMode::Test),
            MenuItem::Action(ActionMode::Install),
            MenuItem::Action(ActionMode::Reset),
            MenuItem::Action(ActionMode::Uninstall),
            MenuItem::Quit,
        ]
    }

    pub fn selected_menu(&self) -> MenuItem {
        let items = Self::menu_items();
        items[self.menu_index.min(items.len() - 1)]
    }

    pub fn refresh(&mut self) {
        // Refresh all state on demand to match the initial load path.
        let (checks, detection, install_state) = Self::load_state();
        self.checks = checks;
        self.detection = detection;
        self.install_state = install_state;
    }

    pub fn build_accel_menu_mode(&self) -> BuildAccelMenuMode {
        let Some(state) = self.build_accel.as_ref() else {
            return BuildAccelMenuMode::ReturnOnly;
        };

        // Enabling only makes sense if at least one accelerator is available.
        let enable_available = state.detection.sccache_installed || state.detection.mold_installed;

        match state.detection.config_status {
            BuildAccelConfigStatus::Managed { .. } => BuildAccelMenuMode::Reinstall,
            BuildAccelConfigStatus::Missing => {
                if enable_available {
                    BuildAccelMenuMode::EnableOrSkip
                } else {
                    BuildAccelMenuMode::ReturnOnly
                }
            }
            BuildAccelConfigStatus::Unmanaged | BuildAccelConfigStatus::ReadFailed(_) => {
                BuildAccelMenuMode::ReturnOnly
            }
        }
    }

    pub fn build_accel_menu_len(&self) -> usize {
        // Keep menu length aligned with the chosen mode to avoid invalid indices.
        match self.build_accel_menu_mode() {
            BuildAccelMenuMode::ReturnOnly => 1,
            BuildAccelMenuMode::EnableOrSkip => 2,
            BuildAccelMenuMode::Reinstall => 2,
        }
    }

    pub fn action_label(&self, mode: ActionMode) -> &'static str {
        match mode {
            ActionMode::Install => self.install_label(),
            ActionMode::Reset => "Reset config",
            _ => mode.label(),
        }
    }

    pub fn refresh_backups(&mut self) {
        // Refresh the list of available backup directories for restore.
        self.restore_backups = crate::actions::list_backup_dirs_for_ui();
        self.restore_menu_index = 0;
    }

    fn install_label(&self) -> &'static str {
        // Installed state is derived from filesystem presence, not runtime health.
        if self
            .install_state
            .as_ref()
            .map(|state| state.is_installed())
            .unwrap_or(false)
        {
            "Reinstall"
        } else {
            "Install"
        }
    }

    fn load_state() -> (Checks, Detection, Option<InstallState>) {
        // Keep initialization and refresh logic consistent in a single helper.
        let checks = Checks::run();
        let detection = crate::detect::detect();
        // Install state remains optional so the UI can render even if discovery fails.
        let install_state = InstallPaths::discover()
            .ok()
            .map(|paths| check_install_state(&paths));
        (checks, detection, install_state)
    }
}
