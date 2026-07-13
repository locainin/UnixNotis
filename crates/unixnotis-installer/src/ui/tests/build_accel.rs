use crate::actions::{BuildAccelConfigStatus, BuildAccelDetection, BuildAccelOutcome};
use crate::app::{BuildAccelState, Screen};
use ratatui::style::{Color, Modifier};

use super::test_support::{app_for_rendering, render_app, render_widget_buffer, style_for_text};

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
fn build_accel_body_reports_missing_detection_state() {
    let mut app = app_for_rendering(Screen::BuildAccel);
    app.build_accel = None;

    let rendered = format!("{:?}", super::build_accel::render_build_accel_body(&app));

    // Missing state should be an explicit diagnostic instead of an empty prompt body
    assert!(rendered.contains("Detection unavailable"));
    assert!(!rendered.contains("sudo pacman"));
}

#[test]
fn build_accel_body_includes_manual_install_hint_when_tools_are_missing() {
    let mut app = app_for_rendering(Screen::BuildAccel);
    app.build_accel = Some(BuildAccelState {
        detection: BuildAccelDetection {
            sccache_installed: false,
            mold_installed: false,
            config_status: BuildAccelConfigStatus::Missing,
        },
        outcome: None,
    });

    let rendered = format!("{:?}", super::build_accel::render_build_accel_body(&app));

    // Installer should suggest packages but must not imply automatic root actions
    assert!(rendered.contains("sudo pacman -S sccache mold"));
    assert!(rendered.contains("Root permissions required by pacman"));
}

#[test]
fn build_accel_body_omits_manual_install_hint_when_both_tools_are_present() {
    let mut app = app_for_rendering(Screen::BuildAccel);
    app.build_accel = Some(BuildAccelState {
        detection: BuildAccelDetection {
            sccache_installed: true,
            mold_installed: true,
            config_status: BuildAccelConfigStatus::Managed {
                wrapper_present: true,
            },
        },
        outcome: None,
    });

    let rendered = format!("{:?}", super::build_accel::render_build_accel_body(&app));

    // Fully available tools should not show a manual package install hint
    assert!(rendered.contains("sccache: "));
    assert!(rendered.contains("mold: "));
    assert!(rendered.contains("installed - sccache + mold"));
    assert!(!rendered.contains("sudo pacman -S sccache mold"));
}

#[test]
fn build_accel_body_reports_partial_tool_status_and_install_hint() {
    let mut app = app_for_rendering(Screen::BuildAccel);
    app.build_accel = Some(BuildAccelState {
        detection: BuildAccelDetection {
            sccache_installed: true,
            mold_installed: false,
            config_status: BuildAccelConfigStatus::Managed {
                wrapper_present: true,
            },
        },
        outcome: None,
    });

    let rendered = format!("{:?}", super::build_accel::render_build_accel_body(&app));

    // One missing tool still needs the install hint while preserving each package row
    assert!(rendered.contains("sccache: "));
    assert!(rendered.contains("mold: "));
    assert!(rendered.contains("installed - sccache"));
    assert!(rendered.contains("sudo pacman -S sccache mold"));
}

#[test]
fn package_status_line_contains_name_status_and_purpose() {
    let installed = format!(
        "{:?}",
        super::build_accel::package_status_line("mold", true, "fast linker")
    );
    let missing = format!(
        "{:?}",
        super::build_accel::package_status_line("sccache", false, "compiler cache")
    );

    // The row is compact but must keep all three pieces of status context
    assert!(installed.contains("mold: "));
    assert!(installed.contains("installed"));
    assert!(installed.contains("fast linker"));
    assert!(missing.contains("sccache: "));
    assert!(missing.contains("missing"));
    assert!(missing.contains("compiler cache"));
}

#[test]
fn build_config_status_line_reports_unmanaged_and_wrapper_missing_states() {
    let unmanaged = BuildAccelDetection {
        sccache_installed: true,
        mold_installed: true,
        config_status: BuildAccelConfigStatus::Unmanaged,
    };
    let wrapper_missing = BuildAccelDetection {
        config_status: BuildAccelConfigStatus::Managed {
            wrapper_present: false,
        },
        ..unmanaged
    };

    let unmanaged_line = format!(
        "{:?}",
        super::build_accel::build_config_status_line(&unmanaged)
    );
    let wrapper_line = format!(
        "{:?}",
        super::build_accel::build_config_status_line(&wrapper_missing)
    );

    // These two states require different user action, so their text must not collapse together
    assert!(unmanaged_line.contains("present (not managed by installer)"));
    assert!(wrapper_line.contains("installed (wrapper missing)"));
}

#[test]
fn build_accel_menu_highlights_selected_choice() {
    let mut app = app_for_rendering(Screen::BuildAccel);
    app.build_accel_menu_index = 1;

    let list = super::build_accel::render_build_accel_menu(&app, 80);
    let buffer = render_widget_buffer(list, 80, 4);
    let selected = style_for_text(&buffer, "Reinstall build acceleration config");
    let unselected = style_for_text(&buffer, "Return to menu");

    // Selection is style-only in the terminal buffer, so assert the exact highlight cells
    assert_eq!(selected.fg, Some(Color::Black));
    assert_eq!(selected.bg, Some(Color::Cyan));
    assert!(selected.add_modifier.contains(Modifier::BOLD));
    assert_ne!(unselected.bg, Some(Color::Cyan));
}

#[test]
fn build_accel_outcome_summary_reports_all_result_variants() {
    let cases = [
        (
            BuildAccelOutcome::SkippedMissingTools,
            "No build accelerators detected; enable unavailable.",
        ),
        (
            BuildAccelOutcome::SkippedExistingConfig,
            "Existing config not managed by installer; no changes applied.",
        ),
        (
            BuildAccelOutcome::Written {
                relative_path: ".cargo/config.toml".to_string(),
                used_sccache: false,
                used_mold: true,
            },
            "Wrote .cargo/config.toml (sccache=false, mold=true).",
        ),
        (
            BuildAccelOutcome::UpdatedExisting {
                relative_path: ".cargo/config.toml".to_string(),
                used_sccache: true,
                used_mold: false,
            },
            "Updated .cargo/config.toml (sccache=true, mold=false).",
        ),
        (
            BuildAccelOutcome::Failed("permission denied".to_string()),
            "Setup failed: permission denied",
        ),
    ];

    for (outcome, expected) in cases {
        assert_eq!(super::build_accel::outcome_summary(&outcome), expected);
    }
}
