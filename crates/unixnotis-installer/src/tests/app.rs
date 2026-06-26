use std::collections::VecDeque;

use crate::actions::{BuildAccelConfigStatus, BuildAccelDetection};
use crate::app::{App, BuildAccelMenuMode, BuildAccelState, MenuItem, ProgressState, Screen};
use crate::checks::{CheckItem, CheckState, Checks};
use crate::detect::Detection;
use crate::model::{ActionMode, ResetAction};

#[test]
fn menu_items_keep_expected_order() {
    let items = App::menu_items();

    // The menu order is muscle memory in the TUI, so accidental reordering should fail loudly
    assert_eq!(items[0], MenuItem::Action(ActionMode::Test));
    assert_eq!(items[1], MenuItem::Action(ActionMode::Install));
    assert_eq!(items[2], MenuItem::Action(ActionMode::Reset));
    assert_eq!(items[3], MenuItem::Action(ActionMode::Uninstall));
    assert_eq!(items[4], MenuItem::Quit);
}

#[test]
fn selected_menu_clamps_out_of_range_index_to_last_item() {
    let mut app = app_with_build_accel(None);
    app.menu_index = usize::MAX;

    // Clamping prevents stale indices from panicking after menu length changes
    assert_eq!(app.selected_menu(), MenuItem::Quit);
}

#[test]
fn build_accel_menu_mode_returns_only_when_no_prompt_state_exists() {
    let app = app_with_build_accel(None);

    // No prompt state means the only valid action is returning to the main menu
    assert_eq!(app.build_accel_menu_mode(), BuildAccelMenuMode::ReturnOnly);
    assert_eq!(app.build_accel_menu_len(), 1);
}

#[test]
fn build_accel_menu_mode_allows_enable_when_tools_exist_and_config_is_missing() {
    let app = app_with_build_accel(Some(BuildAccelDetection {
        sccache_installed: true,
        mold_installed: false,
        config_status: BuildAccelConfigStatus::Missing,
    }));

    // Missing config plus at least one tool gives the user a real enable choice
    assert_eq!(
        app.build_accel_menu_mode(),
        BuildAccelMenuMode::EnableOrSkip
    );
    assert_eq!(app.build_accel_menu_len(), 2);
}

#[test]
fn build_accel_menu_mode_allows_reinstall_for_managed_config() {
    let app = app_with_build_accel(Some(BuildAccelDetection {
        sccache_installed: false,
        mold_installed: false,
        config_status: BuildAccelConfigStatus::Managed {
            wrapper_present: true,
        },
    }));

    // Managed configs can be refreshed even when tool detection changed since the first install
    assert_eq!(app.build_accel_menu_mode(), BuildAccelMenuMode::Reinstall);
    assert_eq!(app.build_accel_menu_len(), 2);
}

#[test]
fn action_label_uses_install_wording_when_state_is_unknown() {
    let app = app_with_build_accel(None);

    // Unknown install state should stay non-destructive in the menu label
    assert_eq!(app.action_label(ActionMode::Install), "Install");
    assert_eq!(app.action_label(ActionMode::Reset), "Reset config");
}

fn app_with_build_accel(detection: Option<BuildAccelDetection>) -> App {
    App {
        checks: passing_checks(),
        detection: Detection {
            owner: None,
            daemons: Vec::new(),
        },
        menu_index: 0,
        screen: Screen::Welcome,
        logs: VecDeque::new(),
        steps: Vec::new(),
        progress_state: ProgressState::Idle,
        last_error: None,
        install_state: None,
        progress_ready_at: None,
        build_accel: detection.map(|detection| BuildAccelState {
            detection,
            outcome: None,
        }),
        build_accel_menu_index: 0,
        reset_menu_index: 0,
        reset_action: ResetAction::ResetDefaults,
        restore_backups: Vec::new(),
        restore_menu_index: 0,
        service_manager: None,
    }
}

fn passing_checks() -> Checks {
    let item = CheckItem {
        label: "test",
        state: CheckState::Ok,
        detail: "ok".to_string(),
    };

    Checks {
        wayland: item.clone(),
        hyprland: item.clone(),
        service_manager: item.clone(),
        cargo: item.clone(),
        pkg_config: item.clone(),
        gtk4_css_features: item.clone(),
        gtk4_layer_shell: item.clone(),
        busctl: item.clone(),
        dbus_update_env: item.clone(),
        install_paths: item.clone(),
        path_contains_bin: item,
    }
}
