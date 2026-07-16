use super::{should_reset_after_action, toggle_action_command};

#[test]
fn toggle_action_command_prefers_custom_toggle_command() {
    let toggle_cmd = "scripts/do-anything".to_string();
    let on_cmd = "turn-on".to_string();
    let off_cmd = "turn-off".to_string();

    assert_eq!(
        toggle_action_command(Some(&toggle_cmd), Some(&on_cmd), Some(&off_cmd), true),
        Some(&toggle_cmd)
    );
    assert_eq!(
        toggle_action_command(Some(&toggle_cmd), Some(&on_cmd), Some(&off_cmd), false),
        Some(&toggle_cmd)
    );
}

#[test]
fn toggle_action_command_uses_on_off_when_custom_command_is_absent() {
    let on_cmd = "turn-on".to_string();
    let off_cmd = "turn-off".to_string();

    assert_eq!(
        toggle_action_command(None, Some(&on_cmd), Some(&off_cmd), true),
        Some(&on_cmd)
    );
    assert_eq!(
        toggle_action_command(None, Some(&on_cmd), Some(&off_cmd), false),
        Some(&off_cmd)
    );
}

#[test]
fn toggle_action_command_allows_state_only_custom_buttons() {
    assert_eq!(toggle_action_command(None, None, None, true), None);
    assert_eq!(toggle_action_command(None, None, None, false), None);
}

#[test]
fn stateless_toggle_command_resets_after_action() {
    let toggle_cmd = "scripts/do-anything".to_string();
    let state_cmd = "scripts/state".to_string();

    assert!(should_reset_after_action(Some(&toggle_cmd), None));
    assert!(!should_reset_after_action(
        Some(&toggle_cmd),
        Some(&state_cmd)
    ));
    assert!(!should_reset_after_action(None, Some(&state_cmd)));
}
