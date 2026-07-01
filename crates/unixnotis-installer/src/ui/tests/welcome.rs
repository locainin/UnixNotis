use crate::app::Screen;
use crate::detect::OwnerInfo;
use ratatui::style::{Color, Modifier};

use super::test_support::{
    app_for_rendering, detected_daemon_with_status, render_app, render_app_buffer, style_for_text,
};

#[test]
fn draw_welcome_renders_status_and_action_menu() {
    let app = app_for_rendering(Screen::Welcome);

    let screen = render_app(&app);

    // Welcome is the first production screen, so empty draws must be caught here
    assert!(screen.contains("UnixNotis Installer"));
    assert!(screen.contains("System status"));
    assert!(screen.contains("Actions"));
    assert!(screen.contains("Compatibility"));
    assert!(screen.contains("[ok]"));
    assert!(screen.contains("test - ok"));
    assert!(screen.contains("Install"));
    assert!(screen.contains("Reset config"));

    let buffer = render_app_buffer(&app);
    let selected = style_for_text(&buffer, "Trial run");
    let unselected = style_for_text(&buffer, "Install");

    // Main menu selection is style-only, so verify it through the terminal buffer
    assert_eq!(selected.fg, Some(Color::Black));
    assert_eq!(selected.bg, Some(Color::Cyan));
    assert!(selected.add_modifier.contains(Modifier::BOLD));
    assert_ne!(unselected.bg, Some(Color::Cyan));
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

    let buffer = render_app_buffer(&app);
    let owner = style_for_text(&buffer, "dunst");
    let status = style_for_text(&buffer, "dbus-owner");

    // Owner and healthy daemon status should use success color, not plain default text
    assert_eq!(owner.fg, Some(Color::Green));
    assert_eq!(status.fg, Some(Color::Green));
}

#[test]
fn draw_welcome_shows_active_daemon_even_without_dbus_owner() {
    let mut app = app_for_rendering(Screen::Welcome);
    app.detection.daemons = vec![detected_daemon_with_status(
        "mako",
        true,
        None,
        Vec::new(),
        false,
    )];

    let screen = render_app(&app);

    // Active service-manager state is enough evidence to show the daemon section
    assert!(screen.contains("Notification daemons"));
    assert!(screen.contains("Owner: none detected"));
    assert!(screen.contains("mako"));
    assert!(screen.contains("systemd-active"));

    let buffer = render_app_buffer(&app);
    let owner = style_for_text(&buffer, "none detected");
    let status = style_for_text(&buffer, "systemd-active");

    // Missing owner is a warning while active service-manager state remains healthy
    assert_eq!(owner.fg, Some(Color::Yellow));
    assert_eq!(status.fg, Some(Color::Green));
}

#[test]
fn draw_welcome_styles_warning_daemon_status_as_warning() {
    let mut app = app_for_rendering(Screen::Welcome);
    app.detection.daemons = vec![detected_daemon_with_status(
        "dunst",
        false,
        Some("systemctl failed".to_string()),
        vec![31337],
        false,
    )];

    let buffer = render_app_buffer(&app);
    let status = style_for_text(&buffer, "systemd-error");

    // Probe errors that are still displayable should stand out as warning state
    assert_eq!(status.fg, Some(Color::Yellow));
}
