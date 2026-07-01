use std::collections::VecDeque;
use std::path::PathBuf;

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use crate::actions::{BuildAccelConfigStatus, BuildAccelDetection, BuildAccelOutcome};
use crate::app::{App, BuildAccelState, ProgressState, Screen};
use crate::checks::{CheckItem, CheckState, Checks};
use crate::detect::{DetectedDaemon, Detection, OwnerInfo};
use crate::model::{ActionMode, ActionStep, ResetAction, StepStatus};

fn app_for_rendering(screen: Screen) -> App {
    // Render tests avoid App::new so they do not depend on the host service manager
    App {
        checks: passing_checks(),
        detection: Detection {
            owner: None,
            daemons: Vec::new(),
        },
        menu_index: 0,
        screen,
        logs: VecDeque::from(["first log".to_string(), "second log".to_string()]),
        steps: vec![
            ActionStep {
                name: "Check existing install",
                status: StepStatus::Done,
            },
            ActionStep {
                name: "Enable user service",
                status: StepStatus::Running,
            },
        ],
        progress_state: ProgressState::Running,
        last_error: Some("command failed: cargo build".to_string()),
        install_state: None,
        progress_ready_at: None,
        build_accel: Some(BuildAccelState {
            detection: BuildAccelDetection {
                sccache_installed: true,
                mold_installed: false,
                config_status: BuildAccelConfigStatus::Managed {
                    wrapper_present: true,
                },
            },
            outcome: Some(BuildAccelOutcome::UpdatedExisting {
                relative_path: ".cargo/config.toml".to_string(),
                used_sccache: true,
                used_mold: false,
            }),
        }),
        build_accel_menu_index: 1,
        reset_menu_index: 0,
        reset_action: ResetAction::RestoreBackup {
            path: PathBuf::from("/tmp/unixnotis-backup"),
        },
        restore_backups: vec![
            PathBuf::from("/tmp/Backup-2026-01-01"),
            PathBuf::from("/tmp/Backup-2026-01-02"),
        ],
        restore_menu_index: 1,
        service_manager: None,
    }
}

fn render_app(app: &App) -> String {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).expect("test backend should initialize");

    terminal
        .draw(|frame| super::draw(frame, app))
        .expect("render should complete");

    // TestBackend display keeps the full screen available for substring checks
    terminal.backend().to_string()
}

#[test]
fn draw_welcome_renders_status_and_action_menu() {
    let app = app_for_rendering(Screen::Welcome);

    let screen = render_app(&app);

    // Welcome is the first production screen, so empty draws must be caught here
    assert!(screen.contains("System status"));
    assert!(screen.contains("Actions"));
    assert!(screen.contains("Install"));
    assert!(screen.contains("Reset config"));
}

#[test]
fn draw_welcome_hides_daemon_section_when_only_probe_errors_exist() {
    let mut app = app_for_rendering(Screen::Welcome);
    app.detection.daemons = vec![detected_daemon_with_status(
        "dunst",
        false,
        Some("systemctl failed".to_string()),
        Vec::new(),
        false,
    )];

    let screen = render_app(&app);

    // Error-only systemd probe noise should not fill the first screen on non-systemd setups
    assert!(!screen.contains("Notification daemons"));
    assert!(!screen.contains("systemctl failed"));
}

#[test]
fn draw_welcome_shows_daemon_section_when_runtime_signal_exists() {
    let mut app = app_for_rendering(Screen::Welcome);
    app.detection.owner = Some(OwnerInfo {
        pid: Some(4242),
        comm: Some("dunst".to_string()),
    });
    app.detection.daemons = vec![detected_daemon_with_status(
        "dunst",
        false,
        None,
        vec![4242],
        true,
    )];

    let screen = render_app(&app);

    // Real ownership or running PIDs still need to be visible for conflict debugging
    assert!(screen.contains("Notification daemons"));
    assert!(screen.contains("Owner: dunst"));
    assert!(screen.contains("dbus-owner"));
    assert!(screen.contains("pid 4242"));
}

#[test]
fn draw_progress_renders_steps_logs_and_error_summary() {
    let mut app = app_for_rendering(Screen::Progress(ActionMode::Install));
    app.progress_state = ProgressState::Failed;

    let screen = render_app(&app);

    // The progress screen must keep both the short error and full logs visible
    assert!(screen.contains("Install - Failed"));
    assert!(screen.contains("Check existing install"));
    assert!(screen.contains("Enable user service"));
    assert!(screen.contains("cargo command failed"));
    assert!(screen.contains("second log"));
}

fn detected_daemon_with_status(
    name: &str,
    systemd_active: bool,
    systemd_error: Option<String>,
    running_pids: Vec<u32>,
    is_owner: bool,
) -> DetectedDaemon {
    DetectedDaemon {
        name: name.to_string(),
        unit: format!("{name}.service"),
        systemd_active,
        systemd_error,
        running_pids,
        is_owner,
    }
}

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
fn draw_build_accel_renders_detection_and_outcome() {
    let app = app_for_rendering(Screen::BuildAccel);

    let screen = render_app(&app);

    // Build acceleration must show both tool detection and the last write result
    assert!(screen.contains("Optional build acceleration"));
    assert!(screen.contains("sccache"));
    assert!(screen.contains("installed"));
    assert!(screen.contains("Updated .cargo/config.toml"));
}

#[test]
fn truncate_to_width_handles_zero_small_and_ellipsis_widths() {
    // Width handling prevents dynamic content from resizing compact list cells
    assert_eq!(super::widgets::truncate_to_width("abcdef", 0), "");
    assert_eq!(super::widgets::truncate_to_width("abcdef", 2), "ab");
    assert_eq!(super::widgets::truncate_to_width("abcdef", 5), "ab...");
    assert_eq!(super::widgets::truncate_to_width("abc", 5), "abc");
}

#[test]
fn summarize_error_prefers_known_short_messages_and_truncates_unknown_errors() {
    let known = super::widgets::summarize_error("command failed: cargo build");
    let unknown = super::widgets::summarize_error(&"x".repeat(80));

    // Known failures stay readable; unknown failures stay bounded for the TUI
    assert_eq!(known, "cargo command failed (see logs)");
    assert_eq!(unknown, format!("{}...", "x".repeat(72)));
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
fn build_accel_outcome_summary_reports_tools_used() {
    let summary = super::build_accel::outcome_summary(&BuildAccelOutcome::Written {
        relative_path: ".cargo/config.toml".to_string(),
        used_sccache: false,
        used_mold: true,
    });

    // Tool flags in the result make debugging partial build acceleration setups easier
    assert_eq!(
        summary,
        "Wrote .cargo/config.toml (sccache=false, mold=true)."
    );
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
