use gtk::prelude::*;
use unixnotis_core::{css::hooks, PanelConfig};

use super::{build_panel_search, SEARCH_REVEAL_TRANSITION_MS};

#[gtk::test]
fn search_widget_applies_visibility_copy_and_transition_policy() {
    let config = PanelConfig {
        search_visible: true,
        search_placeholder: "Find alerts".to_string(),
        search_magnifier_icon: "edit-find-symbolic".to_string(),
        ..PanelConfig::default()
    };

    let search = build_panel_search(&config);

    assert!(search.revealer.reveals_child());
    assert_eq!(
        search.revealer.transition_duration(),
        u32::try_from(SEARCH_REVEAL_TRANSITION_MS).expect("transition fits u32")
    );
    assert_eq!(
        search.entry.placeholder_text().as_deref(),
        Some("Find alerts")
    );
    assert!(search
        .magnifier
        .has_css_class(hooks::panel_shell::SEARCH_MAGNIFIER));
    assert_eq!(
        search.magnifier.icon_name().as_deref(),
        Some("edit-find-symbolic")
    );
    assert!(search
        .entry
        .has_css_class(hooks::panel_shell::SEARCH_OWNED_ICONS));
    assert!(!search.clear_button.get_visible());
}

#[gtk::test]
fn search_clear_action_tracks_and_removes_the_current_query() {
    let search = build_panel_search(&PanelConfig::default());

    search.entry.set_text("urgent");
    assert!(search.clear_button.get_visible());

    search.clear_button.emit_clicked();
    assert!(search.entry.text().is_empty());
    assert!(!search.clear_button.get_visible());
}
