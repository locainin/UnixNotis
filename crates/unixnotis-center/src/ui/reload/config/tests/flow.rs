use std::fs;

use gtk::prelude::*;
use unixnotis_core::{EmptyStateAlignment, Margins, ToggleWidgetConfig};

use super::super::outcome::ConfigReloadOutcome;
use super::support::{state, write_config};

#[gtk::test]
fn reload_config_applies_valid_file_and_rejects_malformed_replacement() {
    let mut state = state();
    let mut reloaded = state.config.clone();
    reloaded.panel.title = "Reloaded from disk".to_string();
    reloaded.panel.footer_label = "Ready".to_string();
    reloaded.panel.empty_alignment = EmptyStateAlignment::Auto;
    reloaded.panel.empty_offset_top = 44;
    reloaded.theme.base_css = "reloaded-base.css".to_string();
    reloaded.widgets.toggles = vec![ToggleWidgetConfig {
        enabled: true,
        kind: Some("test-toggle".to_string()),
        label: "Test Toggle".to_string(),
        ..ToggleWidgetConfig::default()
    }];
    write_config(&state.config_path, &reloaded);
    state.work_area = Some(Margins {
        top: 1,
        right: 2,
        bottom: 3,
        left: 4,
    });

    let outcome = state.reload_config();

    assert!(matches!(outcome, ConfigReloadOutcome::Applied { .. }));
    assert_eq!(state.config.panel.title, "Reloaded from disk");
    assert_eq!(state.panel.header.title.text(), "Reloaded from disk");
    assert_eq!(state.panel.sections.footer.text(), "Ready");
    assert!(state.panel.sections.footer.get_visible());
    assert!(state.toggles.is_some());
    assert!(state
        .panel
        .sections
        .toggle_container
        .first_child()
        .is_some());
    assert_eq!(state.list.empty_overlay.valign(), gtk::Align::Start);
    assert_eq!(state.list.empty_overlay.margin_top(), 44);
    assert!(state.work_area.is_none());
    assert_eq!(
        state.css.theme_paths().base_css,
        state
            .config_path
            .parent()
            .expect("config path should have a parent")
            .join("reloaded-base.css")
    );

    state.widgets_collapsed = true;
    state.apply_list_config_after_reload(&reloaded);
    assert_eq!(state.list.empty_overlay.valign(), gtk::Align::Center);
    assert_eq!(state.list.empty_overlay.margin_top(), 0);

    fs::write(&state.config_path, "[panel\ntitle = broken")
        .expect("malformed config should be written");
    let outcome = state.reload_config();
    assert!(matches!(outcome, ConfigReloadOutcome::Rejected { .. }));
    assert_eq!(state.config.panel.title, "Reloaded from disk");
    assert_eq!(state.panel.header.title.text(), "Reloaded from disk");
    assert!(state.panel.reload_notice.revealer.reveals_child());
    assert!(state
        .panel
        .reload_notice
        .label
        .text()
        .contains("previous configuration is still active"));
    assert!(!state
        .panel
        .reload_notice
        .label
        .text()
        .contains("title = broken"));
}
