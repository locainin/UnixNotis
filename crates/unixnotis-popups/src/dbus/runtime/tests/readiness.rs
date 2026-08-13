use super::super::readiness::wait_for_gtk_runtime;

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
