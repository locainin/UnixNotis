use std::time::Duration;

use crate::control::UiEvent;

pub(super) async fn receive_ui_event(receiver: &async_channel::Receiver<UiEvent>) -> UiEvent {
    // Missing publication should fail at the assertion instead of holding the full CI job
    tokio::time::timeout(Duration::from_secs(2), receiver.recv())
        .await
        .expect("media event should arrive promptly")
        .expect("media event channel should remain open")
}
