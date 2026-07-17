use super::ControlGenerationExit;

#[test]
fn only_a_disconnected_live_generation_requests_reconnect_cleanup() {
    assert!(ControlGenerationExit::Disconnected.requires_reconnect_cleanup());
    assert!(!ControlGenerationExit::RetryDelayed.requires_reconnect_cleanup());
    assert!(!ControlGenerationExit::Shutdown.requires_reconnect_cleanup());
}

#[test]
fn only_a_closed_ui_command_channel_stops_the_control_task() {
    assert!(ControlGenerationExit::Shutdown.should_stop());
    assert!(!ControlGenerationExit::Disconnected.should_stop());
    assert!(!ControlGenerationExit::RetryDelayed.should_stop());
}
