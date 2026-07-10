use std::time::Duration;

#[tokio::test]
async fn shutdown_signal_waits_when_no_signal_arrives() {
    let result = tokio::time::timeout(Duration::from_millis(25), super::shutdown_signal()).await;

    assert!(result.is_err());
}
