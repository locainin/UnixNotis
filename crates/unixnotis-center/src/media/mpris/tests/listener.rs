use std::collections::HashMap;
use std::time::Duration;

use tokio::sync::mpsc;
use unixnotis_core::MediaConfig;

use super::super::listener::{is_relevant_media_change, spawn_properties_listener};
use super::super::player::build_player_state;
use super::support::{MprisFixture, TEST_PLAYER_NAME};
use crate::media::runtime::{MediaRefreshOrigin, MediaSignal};

#[test]
fn relevant_media_change_detects_updates_and_invalidations() {
    let mut changed = HashMap::new();
    changed.insert("Metadata", zbus::zvariant::Value::from("track"));
    let no_invalidations: [&str; 0] = [];

    assert!(is_relevant_media_change(&changed, &no_invalidations));

    let no_changes: HashMap<&str, zbus::zvariant::Value<'_>> = HashMap::new();
    assert!(is_relevant_media_change(&no_changes, &["CanPlay"]));
}

#[test]
fn relevant_media_change_ignores_unrelated_properties() {
    let mut changed = HashMap::new();
    changed.insert("Volume", zbus::zvariant::Value::from(0.5_f64));

    assert!(!is_relevant_media_change(&changed, &["Position"]));
}

#[tokio::test]
async fn property_listener_forwards_relevant_live_player_changes() {
    let fixture = MprisFixture::start().await;
    let state = build_player_state(&fixture.client, TEST_PLAYER_NAME, &MediaConfig::default())
        .await
        .expect("probe test MPRIS player")
        .expect("stable test MPRIS owner");
    let (signal_tx, mut signal_rx) = mpsc::channel(4);
    let cancel_tx = state.listener_cancel.clone();
    spawn_properties_listener(
        state.properties,
        TEST_PLAYER_NAME.to_string(),
        signal_tx,
        cancel_tx.subscribe(),
    );

    let signal = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            fixture.emit_playback_status_changed().await;
            if let Ok(signal) = signal_rx.try_recv() {
                break signal;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("listener should forward a property change");

    assert!(matches!(
        signal,
        MediaSignal::PropertiesChanged {
            bus_name,
            origin: MediaRefreshOrigin::Bus,
        } if bus_name == TEST_PLAYER_NAME
    ));
    let _ = cancel_tx.send(true);
}
