use std::collections::HashMap;
use std::time::Duration;

use super::super::refresh::prune_player_refreshes;
use super::super::refresh::refresh_all_players;
use super::super::state::MediaRuntimeState;
use crate::control::UiEvent;
use crate::media::mpris::tests::support::{MprisFixture, TEST_PLAYER_NAME};
use unixnotis_core::MediaConfig;
use zbus::fdo::DBusProxy;

#[tokio::test]
async fn refresh_pruning_removes_delayed_work_for_missing_players() {
    let mut delayed = HashMap::new();
    delayed.insert(
        "org.mpris.MediaPlayer2.gone".to_string(),
        tokio::spawn(std::future::pending()),
    );
    let players = HashMap::new();

    prune_player_refreshes(&mut delayed, &players);

    assert!(delayed.is_empty());
}

#[tokio::test]
async fn full_refresh_discovers_caches_and_publishes_live_players() {
    let fixture = MprisFixture::start().await;
    let proxy = DBusProxy::new(&fixture.client)
        .await
        .expect("create private bus proxy");
    let (signal_tx, _signal_rx) = tokio::sync::mpsc::channel(8);
    let (event_tx, event_rx) = async_channel::bounded(4);
    let mut state = MediaRuntimeState::new();

    tokio::time::timeout(
        Duration::from_secs(2),
        refresh_all_players(
            &fixture.client,
            &proxy,
            &MediaConfig::default(),
            &signal_tx,
            &mut state,
            &event_tx,
        ),
    )
    .await
    .expect("full media refresh should stay bounded");

    assert!(state.players.contains_key(TEST_PLAYER_NAME));
    assert!(state.cache.contains_key(TEST_PLAYER_NAME));
    assert_eq!(state.last_snapshot.len(), 1);
    assert!(matches!(
        event_rx.recv().await,
        Ok(UiEvent::MediaUpdated(infos)) if infos[0].bus_name == TEST_PLAYER_NAME
    ));
    let _ = state.players[TEST_PLAYER_NAME].listener_cancel.send(true);
    for (_, task) in state.delayed_refreshes {
        task.abort();
    }
}
