use std::collections::VecDeque;
use std::path::PathBuf;

use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Style;
use ratatui::widgets::Widget;
use ratatui::Terminal;

use crate::actions::{BuildAccelConfigStatus, BuildAccelDetection, BuildAccelOutcome};
use crate::app::{App, BuildAccelState, ProgressState, Screen};
use crate::checks::{CheckItem, CheckState, Checks};
use crate::detect::{DetectedDaemon, Detection};
use crate::model::{ActionStep, ResetAction, StepStatus};

pub(super) fn app_for_rendering(screen: Screen) -> App {
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
        release_status: crate::release::ReleaseStatus::current_only(),
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

pub(super) fn render_app(app: &App) -> String {
    buffer_text(&render_app_buffer(app))
}

pub(super) fn render_app_buffer(app: &App) -> Buffer {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).expect("test backend should initialize");

    terminal
        .draw(|frame| super::draw(frame, app))
        .expect("render should complete");

    // Buffer access lets tests assert styles without depending on terminal escape codes
    terminal.backend().buffer().clone()
}

pub(super) fn render_widget_buffer(widget: impl Widget, width: u16, height: u16) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test backend should initialize");

    terminal
        .draw(|frame| frame.render_widget(widget, frame.area()))
        .expect("widget render should complete");

    // Widget-level tests stay small and avoid hard-coding full installer screens
    terminal.backend().buffer().clone()
}

pub(super) fn style_for_text(buffer: &Buffer, needle: &str) -> Style {
    let (x, y) = find_text_start(buffer, needle)
        .unwrap_or_else(|| panic!("rendered buffer should contain {needle:?}"));

    buffer
        .cell((x, y))
        .unwrap_or_else(|| panic!("rendered buffer cell should exist for {needle:?}"))
        .style()
}

pub(super) fn buffer_text(buffer: &Buffer) -> String {
    let mut out = String::new();

    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            // The test buffer stores symbols per cell, which is enough for ASCII UI assertions
            if let Some(cell) = buffer.cell((x, y)) {
                out.push_str(cell.symbol());
            }
        }
        out.push('\n');
    }

    out
}

fn find_text_start(buffer: &Buffer, needle: &str) -> Option<(u16, u16)> {
    for y in 0..buffer.area.height {
        let mut row = String::new();

        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y))?;
            row.push_str(cell.symbol());
        }

        if let Some(index) = row.find(needle) {
            // Convert byte offset to rendered-cell offset so multibyte labels stay valid
            let x = row[..index].chars().count();
            let x = u16::try_from(x).ok()?;
            return Some((x, y));
        }
    }

    None
}

pub(super) fn detected_daemon_with_status(
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

fn passing_checks() -> Checks {
    let item = CheckItem {
        label: "test",
        state: CheckState::Ok,
        detail: "ok".to_string(),
    };

    Checks {
        release_archive: false,
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
