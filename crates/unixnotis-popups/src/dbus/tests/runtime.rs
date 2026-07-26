use super::{build_runtime, wait_for_gtk_runtime, UI_COMMAND_QUEUE_CAPACITY};

#[test]
fn popup_runtime_builds_with_a_bounded_command_queue() {
    assert!(build_runtime().is_some());
    assert_eq!(UI_COMMAND_QUEUE_CAPACITY, 64);
}

#[tokio::test]
async fn gtk_readiness_wait_completes_after_ui_state_is_published() {
    let (ready_tx, mut ready_rx) = tokio::sync::watch::channel(false);
    ready_tx.send(true).expect("publish GTK readiness");

    assert!(wait_for_gtk_runtime(&mut ready_rx).await);
}

#[tokio::test]
async fn gtk_readiness_wait_rejects_a_closed_unready_startup_channel() {
    let (ready_tx, mut ready_rx) = tokio::sync::watch::channel(false);
    drop(ready_tx);

    assert!(!wait_for_gtk_runtime(&mut ready_rx).await);
}
