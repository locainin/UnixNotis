use super::super::state::MediaRuntimeState;
use std::time::Duration;

#[test]
fn new_runtime_state_starts_without_players_cache_or_delayed_work() {
    let state = MediaRuntimeState::new();

    assert!(state.players.is_empty());
    assert!(state.cache.is_empty());
    assert!(state.last_snapshot.is_empty());
    assert!(state.delayed_refreshes.is_empty());
}

#[tokio::test]
async fn dropping_runtime_state_aborts_delayed_refresh_tasks() {
    let (completed_tx, completed_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_mins(1)).await;
        let _ = completed_tx.send(());
    });
    let mut state = MediaRuntimeState::new();
    state
        .delayed_refreshes
        .insert("org.mpris.MediaPlayer2.test".to_string(), task);

    drop(state);

    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(1), completed_rx).await,
        Ok(Err(_))
    ));
}
