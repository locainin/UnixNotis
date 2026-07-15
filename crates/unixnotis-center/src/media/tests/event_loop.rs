use super::{send_owner_rebuild_retry_after, OWNER_REBUILD_RETRY_MS};

#[tokio::test]
async fn owner_rebuild_retry_emits_one_delayed_refresh_signal() {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);

    send_owner_rebuild_retry_after(std::time::Duration::from_millis(1), sender).await;

    assert_eq!(receiver.recv().await, Some(()));
    assert!(receiver.try_recv().is_err(), "retry must emit only once");
    assert_eq!(OWNER_REBUILD_RETRY_MS, 200);
}
