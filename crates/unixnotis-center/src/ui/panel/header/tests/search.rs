use unixnotis_core::PanelConfig;

use super::{build_panel_search, SEARCH_REVEAL_TRANSITION_MS};

#[gtk::test]
fn search_widget_applies_visibility_copy_and_transition_policy() {
    let config = PanelConfig {
        search_visible: true,
        search_placeholder: "Find alerts".to_string(),
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
}
