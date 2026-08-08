use std::collections::HashMap;
use std::time::Duration;

use tokio::sync::mpsc;
use unixnotis_core::MediaConfig;
use zbus::fdo::DBusProxy;

use super::super::discovery::refresh_players;
use super::super::fairness::MprisFairnessState;
use super::support::{build_player_state, MprisFixture, TEST_PLAYER_IDENTITY, TEST_PLAYER_NAME};

#[tokio::test]
async fn discovery_adds_live_players_and_removes_stale_entries() {
    let fixture = MprisFixture::start().await;
    let config = MediaConfig::default();
    let dbus_proxy = DBusProxy::new(&fixture.client)
        .await
        .expect("create private bus proxy");
    let (signal_tx, _signal_rx) = mpsc::channel(4);
    let mut stale = build_player_state(&fixture.client, TEST_PLAYER_NAME, &config)
        .await
        .expect("probe test MPRIS player")
        .expect("stable test MPRIS owner");
    let stale_name = "org.mpris.MediaPlayer2.stale";
    stale.bus_name = stale_name.to_string();
    let mut stale_cancel = stale.listener_cancel.subscribe();
    let mut players = HashMap::from([(stale_name.to_string(), stale)]);
    let mut discovery_cursor = 0;
    let mut fairness = MprisFairnessState::new();

    tokio::time::timeout(
        Duration::from_secs(2),
        refresh_players(
            &fixture.client,
            &dbus_proxy,
            &config,
            &signal_tx,
            &mut players,
            &mut discovery_cursor,
            &mut fairness,
        ),
    )
    .await
    .expect("player discovery should stay bounded")
    .expect("refresh private MPRIS players");

    assert!(!players.contains_key(stale_name));
    assert_eq!(players[TEST_PLAYER_NAME].identity, TEST_PLAYER_IDENTITY);
    stale_cancel
        .changed()
        .await
        .expect("stale listener cancellation");
    assert!(*stale_cancel.borrow());
    let _ = players[TEST_PLAYER_NAME].listener_cancel.send(true);
}
