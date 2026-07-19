use unixnotis_core::PanelAction;

use super::panel_visibility_for_action;

#[test]
fn panel_actions_resolve_open_close_and_both_toggle_edges() {
    assert!(panel_visibility_for_action(false, PanelAction::Open));
    assert!(panel_visibility_for_action(true, PanelAction::Open));
    assert!(!panel_visibility_for_action(false, PanelAction::Close));
    assert!(!panel_visibility_for_action(true, PanelAction::Close));
    assert!(panel_visibility_for_action(false, PanelAction::Toggle));
    assert!(!panel_visibility_for_action(true, PanelAction::Toggle));
}
