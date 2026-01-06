//! UI state and event handling for the installer TUI.

use crate::actions::{check_install_state, InstallState};
use crate::checks::Checks;
use crate::detect::Detection;
use crate::model::{ActionMode, ActionStep};
use crate::paths::InstallPaths;
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
    // Progress screen for running actions.
    Progress(ActionMode),
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

    // Whether to run extra verification steps.
    pub verify: bool,

    // Log lines for UI display.
    pub logs: Vec<String>,

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
}

impl App {
    pub fn new() -> Self {
        // Initialize with current system state.
        let checks = Checks::run();
        let detection = crate::detect::detect();
        let install_state = InstallPaths::discover()
            .ok()
            .map(|paths| check_install_state(&paths));

        Self {
            checks,
            detection,
            menu_index: 0,
            screen: Screen::Welcome,
            verify: false,
            logs: Vec::new(),
            steps: Vec::new(),
            progress_state: ProgressState::Idle,
            last_error: None,
            install_state,
            progress_ready_at: None,
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
        self.checks = Checks::run();
        self.detection = crate::detect::detect();
        self.install_state = InstallPaths::discover()
            .ok()
            .map(|paths| check_install_state(&paths));
    }

    pub fn action_label(&self, mode: ActionMode) -> &'static str {
        match mode {
            ActionMode::Install => self.install_label(),
            ActionMode::Reset => "Reset config",
            _ => mode.label(),
        }
    }

    fn install_label(&self) -> &'static str {
        if self
            .install_state
            .as_ref()
            .map(|state| state.is_fully_installed())
            .unwrap_or(false)
        {
            "Reinstall"
        } else {
            "Install"
        }
    }
}
