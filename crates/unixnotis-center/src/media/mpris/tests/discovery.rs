use std::collections::HashMap;
use std::time::Duration;

use tokio::sync::mpsc;
use unixnotis_core::MediaConfig;
use zbus::fdo::DBusProxy;

use super::super::discovery::{is_discoverable_player, refresh_players};
use super::super::player::build_player_state;
use super::support::{MprisFixture, TEST_PLAYER_IDENTITY, TEST_PLAYER_NAME};

#[test]
fn discovery_requires_an_mpris_name_that_passes_admission() {
    let config = MediaConfig {
        denylist: vec!["blocked".to_string()],
        ..MediaConfig::default()
    };

    assert!(is_discoverable_player(
        "org.mpris.MediaPlayer2.allowed",
        &config
    ));
    assert!(!is_discoverable_player("org.example.allowed", &config));
    assert!(!is_discoverable_player(
        "org.mpris.MediaPlayer2.blocked",
        &config
    ));
}

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

    tokio::time::timeout(
        Duration::from_secs(2),
        refresh_players(
            &fixture.client,
            &dbus_proxy,
            &config,
            &signal_tx,
            &mut players,
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
