use super::SeedError;

#[test]
fn seed_error_keeps_handshake_snapshot_and_delivery_failures_distinct() {
    let error = SeedError {
        state_error: Some("state unavailable".to_string()),
        active_error: None,
        history_error: None,
        send_error: None,
    };

    assert!(error.state_error.is_some());
    assert!(error.active_error.is_none());
    assert!(error.history_error.is_none());
    assert!(error.send_error.is_none());
}
