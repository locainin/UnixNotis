use super::ControlGenerationExit;

#[test]
fn only_a_disconnected_live_generation_requests_reconnect_cleanup() {
    assert!(ControlGenerationExit::Disconnected.requires_reconnect_cleanup());
    assert!(!ControlGenerationExit::RetryDelayed.requires_reconnect_cleanup());
}
