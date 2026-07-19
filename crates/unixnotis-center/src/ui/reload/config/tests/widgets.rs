use gtk::prelude::*;
use unixnotis_core::EmptyStateAlignment;

use super::support::state;

#[gtk::test]
fn reloaded_list_applies_explicit_empty_alignment() {
    let mut state = state();
    let mut config = state.config.clone();
    config.panel.empty_text = "Nothing pending".to_string();
    config.panel.empty_alignment = EmptyStateAlignment::End;
    config.panel.empty_offset_top = 44;

    state.apply_list_config_after_reload(&config);

    assert_eq!(state.list.empty_text, "Nothing pending");
    assert_eq!(state.list.empty_overlay.valign(), gtk::Align::End);
    assert_eq!(state.list.empty_overlay.margin_top(), 0);
}
