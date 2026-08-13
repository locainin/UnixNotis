use super::ControlGenerationExit;

#[test]
fn only_a_broken_bus_generation_uses_connection_backoff() {
    assert!(ControlGenerationExit::Disconnected.requires_connection_backoff());
    assert!(!ControlGenerationExit::OwnerChanged.requires_connection_backoff());
    assert!(!ControlGenerationExit::RetryDelayed.requires_connection_backoff());
    assert!(!ControlGenerationExit::Shutdown.requires_connection_backoff());
}

#[test]
fn only_a_closed_ui_command_channel_stops_the_control_task() {
    assert!(ControlGenerationExit::Shutdown.should_stop());
    assert!(!ControlGenerationExit::Disconnected.should_stop());
    assert!(!ControlGenerationExit::OwnerChanged.should_stop());
    assert!(!ControlGenerationExit::RetryDelayed.should_stop());
}

#[test]
fn owner_loss_never_calls_the_panel_readiness_cleanup_method() {
    assert!(ControlGenerationExit::Shutdown.should_clear_panel_readiness());
    assert!(!ControlGenerationExit::Disconnected.should_clear_panel_readiness());
    assert!(!ControlGenerationExit::OwnerChanged.should_clear_panel_readiness());
    assert!(!ControlGenerationExit::RetryDelayed.should_clear_panel_readiness());
}
