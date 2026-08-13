use std::collections::HashMap;
use std::time::Duration;

use tokio::sync::mpsc;
use unixnotis_core::MediaConfig;
use zbus::Message;

use super::super::constants::{
    MAX_MPRIS_CHANGED_PROPERTIES, MAX_MPRIS_PROPERTIES_CHANGED_BODY_BYTES, MPRIS_PATH, MPRIS_PLAYER,
};
use super::super::listener::{
    changed_property_count_allowed, is_relevant_media_change, properties_changed_body_allowed,
    relevant_media_change_from_message, spawn_properties_listener,
};
use super::support::{build_player_state, MprisFixture, TEST_PLAYER_NAME};
use crate::media::runtime::{MediaRefreshOrigin, MediaSignal};

fn properties_changed_message<T>(body: &T) -> Message
where
    T: serde::Serialize + zbus::zvariant::DynamicType,
{
    Message::signal(
        MPRIS_PATH,
        "org.freedesktop.DBus.Properties",
        "PropertiesChanged",
    )
    .expect("property signal builder")
    .build(body)
    .expect("property signal message")
}

fn signal_with_exact_body_len(target_len: usize) -> Message {
    let empty = properties_changed_message(&(Vec::<u8>::new(),));
    let overhead = empty.body().len();
    let payload_len = target_len
        .checked_sub(overhead)
        .expect("target body must exceed the encoded array overhead");
    let message = properties_changed_message(&(vec![0_u8; payload_len],));
    assert_eq!(message.body().len(), target_len);
    message
}

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

#[test]
fn properties_changed_encoded_body_limit_accepts_only_the_exact_budget() {
    assert!(properties_changed_body_allowed(
        MAX_MPRIS_PROPERTIES_CHANGED_BODY_BYTES - 1
    ));
    assert!(properties_changed_body_allowed(
        MAX_MPRIS_PROPERTIES_CHANGED_BODY_BYTES
    ));
    assert!(!properties_changed_body_allowed(
        MAX_MPRIS_PROPERTIES_CHANGED_BODY_BYTES + 1
    ));
}

#[test]
fn raw_properties_changed_gate_handles_encoded_bodies_on_both_sides_of_limit() {
    let below = signal_with_exact_body_len(MAX_MPRIS_PROPERTIES_CHANGED_BODY_BYTES - 1);
    let above = signal_with_exact_body_len(MAX_MPRIS_PROPERTIES_CHANGED_BODY_BYTES + 1);

    assert!(properties_changed_body_allowed(below.body().len()));
    assert!(!properties_changed_body_allowed(above.body().len()));
    assert_eq!(relevant_media_change_from_message(&above), None);
}

#[test]
fn properties_changed_entry_limit_bounds_changes_and_invalidations_together() {
    assert!(changed_property_count_allowed(
        MAX_MPRIS_CHANGED_PROPERTIES,
        0
    ));
    assert!(changed_property_count_allowed(16, 16));
    assert!(!changed_property_count_allowed(16, 17));
    assert!(!changed_property_count_allowed(usize::MAX, 1));
}

#[test]
fn raw_properties_changed_decoder_accepts_normal_media_signals() {
    let changed = HashMap::from([("Metadata", zbus::zvariant::Value::from("track"))]);
    let message = properties_changed_message(&(MPRIS_PLAYER, changed, Vec::<&str>::new()));

    assert_eq!(relevant_media_change_from_message(&message), Some(true));
}

#[test]
fn raw_properties_changed_decoder_rejects_oversized_irrelevant_data_before_decode() {
    let oversized = "x".repeat(MAX_MPRIS_PROPERTIES_CHANGED_BODY_BYTES + 1_024);
    let changed = HashMap::from([("Unrelated", zbus::zvariant::Value::from(oversized.as_str()))]);
    let message = properties_changed_message(&(MPRIS_PLAYER, changed, Vec::<&str>::new()));

    assert!(message.body().len() > MAX_MPRIS_PROPERTIES_CHANGED_BODY_BYTES);
    assert_eq!(relevant_media_change_from_message(&message), None);
}

#[test]
fn raw_properties_changed_decoder_rejects_malformed_body() {
    let message = properties_changed_message(&("wrong shape",));

    assert_eq!(relevant_media_change_from_message(&message), None);
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
