use std::fs;

use gtk::prelude::*;

use super::super::outcome::ConfigReloadOutcome;
use super::support::{enable_missing_panel_layer_fixture, state, write_config};

#[gtk::test]
fn accepted_reload_clears_rejected_config_notice() {
    let mut state = state();
    fs::write(&state.config_path, "[panel\ntitle = broken").expect("broken config");
    let _outcome = state.reload_config();
    assert!(state.panel.reload_notice.revealer.reveals_child());

    let valid = state.config.clone();
    write_config(&state.config_path, &valid);
    let theme_paths = valid
        .resolve_theme_paths_from(state.config_path.parent().expect("config parent"))
        .expect("theme paths");
    for path in [
        theme_paths.base_css,
        theme_paths.panel_css,
        theme_paths.widgets_css,
        theme_paths.media_css,
    ] {
        fs::write(path, "/* intentionally valid */").expect("theme css");
    }

    let outcome = state.reload_config();

    assert!(matches!(outcome, ConfigReloadOutcome::Applied { .. }));
    assert!(!state.panel.reload_notice.revealer.reveals_child());
}

#[gtk::test]
fn dismissed_reload_notice_stays_hidden_until_failure_fingerprint_changes() {
    let mut state = state();
    fs::write(&state.config_path, "[panel\ntitle = first").expect("first broken config");
    let _outcome = state.reload_config();
    assert!(state.panel.reload_notice.revealer.reveals_child());

    state.panel.reload_notice.close.emit_clicked();
    assert!(!state.panel.reload_notice.revealer.reveals_child());

    let _same_outcome = state.reload_config();
    assert!(!state.panel.reload_notice.revealer.reveals_child());

    fs::write(&state.config_path, "config_version = 999").expect("distinct broken config");
    let _distinct_outcome = state.reload_config();
    assert!(state.panel.reload_notice.revealer.reveals_child());
}

#[gtk::test]
fn changed_css_failure_reopens_after_the_previous_failure_was_dismissed() {
    let mut state = state();
    enable_missing_panel_layer_fixture(&mut state);
    let first_report = state.reload_css();
    assert!(first_report.read_failures().count() > 1);
    assert!(state.panel.reload_notice.revealer.reveals_child());

    state.panel.reload_notice.close.emit_clicked();
    assert!(!state.panel.reload_notice.revealer.reveals_child());

    let same_report = state.reload_css();
    assert!(same_report.read_failures().count() > 1);
    assert!(!state.panel.reload_notice.revealer.reveals_child());

    let theme_paths = state
        .config
        .resolve_theme_paths_from(state.config_path.parent().expect("config parent"))
        .expect("theme paths");
    fs::write(theme_paths.base_css, "/* one layer recovered */").expect("base theme css");

    let changed_report = state.reload_css();
    assert!(changed_report.read_failures().count() > 0);
    assert!(state.panel.reload_notice.revealer.reveals_child());
}

#[gtk::test]
fn successful_css_only_reload_does_not_clear_config_rejection_notice() {
    let mut state = state();
    let theme_paths = state
        .config
        .resolve_theme_paths_from(state.config_path.parent().expect("config parent"))
        .expect("theme paths");
    for path in [
        theme_paths.base_css,
        theme_paths.panel_css,
        theme_paths.widgets_css,
        theme_paths.media_css,
    ] {
        fs::write(path, "/* valid reload css */").expect("theme css");
    }
    fs::write(&state.config_path, "[panel\ntitle = broken").expect("broken config");
    let _outcome = state.reload_config();
    let rejection = state.panel.reload_notice.label.text();

    let report = state.reload_css();

    assert_eq!(report.read_failures().count(), 0);
    assert!(state.panel.reload_notice.revealer.reveals_child());
    assert_eq!(state.panel.reload_notice.label.text(), rejection);
}

#[gtk::test]
fn css_failure_cannot_replace_an_active_config_rejection() {
    let mut state = state();
    enable_missing_panel_layer_fixture(&mut state);
    fs::write(&state.config_path, "[panel\ntitle = broken").expect("broken config");
    let _outcome = state.reload_config();
    let rejection = state.panel.reload_notice.label.text();

    let report = state.reload_css();

    assert!(report.read_failures().count() > 0);
    assert!(state.panel.reload_notice.revealer.reveals_child());
    assert_eq!(state.panel.reload_notice.label.text(), rejection);
    assert!(state
        .panel
        .reload_notice
        .shell
        .has_css_class(unixnotis_core::css::hooks::panel_shell::RELOAD_NOTICE_ERROR));
}

#[gtk::test]
fn css_reload_notice_summarizes_multiple_unreadable_layers() {
    let mut state = state();
    enable_missing_panel_layer_fixture(&mut state);
    let report = state.reload_css();

    assert!(report.read_failures().count() > 1);
    assert!(state.panel.reload_notice.revealer.reveals_child());
    assert!(state
        .panel
        .reload_notice
        .label
        .text()
        .contains("other layer"));
    assert!(state
        .panel
        .reload_notice
        .shell
        .has_css_class(unixnotis_core::css::hooks::panel_shell::RELOAD_NOTICE_WARNING));
}
