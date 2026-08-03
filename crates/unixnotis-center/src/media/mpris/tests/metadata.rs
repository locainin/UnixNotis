use super::super::constants::MAX_MPRIS_PROPERTY_REPLY_BYTES;
use super::super::metadata::fetch_media_info;
use super::super::metadata::{
    bound_string, is_plasma_browser_bridge, metadata_artist, metadata_entry_count_allowed,
    metadata_pid, metadata_string, property_reply_body_allowed,
};
use super::super::player::build_player_state;
use super::support::{MprisFixture, TEST_BRIDGE_PLAYER_NAME, TEST_PLAYER_NAME};
use unixnotis_core::MediaConfig;
use zbus::zvariant::{OwnedValue, Value};

const SOURCE_BROWSER_PID: u32 = 42_424;

#[test]
fn bounded_metadata_strings_trim_and_preserve_utf8_boundaries() {
    assert_eq!(bound_string("  title  ", 32), "title");
    assert_eq!(bound_string("éé", 3), "é");
    assert_eq!(bound_string("title", 0), "");
}

#[test]
fn metadata_fields_accept_expected_string_shapes() {
    let title = OwnedValue::try_from(Value::from("A title")).expect("title value");
    let artists =
        OwnedValue::try_from(Value::from(vec!["Artist".to_string()])).expect("artist value");
    let metadata = std::collections::HashMap::from([
        ("xesam:title".to_string(), title),
        ("xesam:artist".to_string(), artists),
    ]);

    assert_eq!(
        metadata_string(&metadata, "xesam:title").as_deref(),
        Some("A title")
    );
    assert_eq!(metadata_artist(&metadata).as_deref(), Some("Artist"));
}

#[test]
fn metadata_pid_accepts_unsigned_values_and_rejects_negative_values() {
    let positive = std::collections::HashMap::from([(
        "kde:pid".to_string(),
        OwnedValue::from(SOURCE_BROWSER_PID),
    )]);
    let negative =
        std::collections::HashMap::from([("kde:pid".to_string(), OwnedValue::from(-1_i32))]);
    let zero = std::collections::HashMap::from([("kde:pid".to_string(), OwnedValue::from(0_u32))]);

    assert_eq!(metadata_pid(&positive, "kde:pid"), Some(SOURCE_BROWSER_PID));
    assert_eq!(metadata_pid(&negative, "kde:pid"), None);
    assert_eq!(metadata_pid(&zero, "kde:pid"), None);
}

#[test]
fn metadata_pid_accepts_positive_signed_values() {
    let positive = std::collections::HashMap::from([(
        "kde:pid".to_string(),
        OwnedValue::from(i32::try_from(SOURCE_BROWSER_PID).expect("fixture PID fits")),
    )]);

    assert_eq!(metadata_pid(&positive, "kde:pid"), Some(SOURCE_BROWSER_PID));
}

#[test]
fn source_pid_hints_are_limited_to_plasma_bridge_names() {
    assert!(is_plasma_browser_bridge(TEST_BRIDGE_PLAYER_NAME));
    assert!(is_plasma_browser_bridge(
        "org.mpris.MediaPlayer2.plasma-browser-integration.instance"
    ));
    assert!(!is_plasma_browser_bridge(TEST_PLAYER_NAME));
    assert!(!is_plasma_browser_bridge(
        "org.mpris.MediaPlayer2.other-browser-bridge"
    ));
    assert!(!is_plasma_browser_bridge(
        "org.mpris.MediaPlayer2.plasma-browser-integration-fake"
    ));
}

#[test]
fn metadata_artist_rejects_empty_and_oversized_artist_lists() {
    let empty =
        OwnedValue::try_from(Value::from(vec![" ".to_string()])).expect("empty artist value");
    let oversized = OwnedValue::try_from(Value::from(
        (0..17)
            .map(|index| format!("Artist {index}"))
            .collect::<Vec<_>>(),
    ))
    .expect("oversized artist value");
    let maximum = OwnedValue::try_from(Value::from(
        (0..16)
            .map(|index| format!("Artist {index}"))
            .collect::<Vec<_>>(),
    ))
    .expect("maximum artist value");
    let scalar = OwnedValue::try_from(Value::from("Solo artist")).expect("scalar artist value");

    let empty_metadata = std::collections::HashMap::from([("xesam:artist".to_string(), empty)]);
    let oversized_metadata =
        std::collections::HashMap::from([("xesam:artist".to_string(), oversized)]);
    let maximum_metadata = std::collections::HashMap::from([("xesam:artist".to_string(), maximum)]);
    let scalar_metadata = std::collections::HashMap::from([("xesam:artist".to_string(), scalar)]);

    assert_eq!(metadata_artist(&empty_metadata), None);
    assert_eq!(metadata_artist(&oversized_metadata), None);
    assert_eq!(
        metadata_artist(&maximum_metadata).as_deref(),
        Some("Artist 0")
    );
    assert_eq!(
        metadata_artist(&scalar_metadata).as_deref(),
        Some("Solo artist")
    );
}

#[test]
fn metadata_limits_accept_exact_boundaries_only() {
    assert_eq!(MAX_MPRIS_PROPERTY_REPLY_BYTES, 512 * 1024);
    assert!(metadata_entry_count_allowed(256));
    assert!(!metadata_entry_count_allowed(257));
    assert!(property_reply_body_allowed(MAX_MPRIS_PROPERTY_REPLY_BYTES));
    assert!(!property_reply_body_allowed(
        MAX_MPRIS_PROPERTY_REPLY_BYTES + 1
    ));
}

#[tokio::test]
async fn oversized_metadata_reply_is_rejected_before_dynamic_decode() {
    let fixture = MprisFixture::start_with_metadata_bytes(MAX_MPRIS_PROPERTY_REPLY_BYTES + 1).await;
    let player = build_player_state(&fixture.client, TEST_PLAYER_NAME, &MediaConfig::default())
        .await
        .expect("build oversized-metadata fixture player")
        .expect("fixture owner should remain stable");

    let info = fetch_media_info(&player)
        .await
        .expect("required playback status remains available");
    assert!(info.title.is_empty());
    assert!(info.artist.is_empty());
}

#[tokio::test]
async fn oversized_art_url_is_not_retained() {
    let fixture = MprisFixture::start_with_art_url_bytes(2_049).await;
    let player = build_player_state(&fixture.client, TEST_PLAYER_NAME, &MediaConfig::default())
        .await
        .expect("build oversized-art fixture player")
        .expect("fixture owner should remain stable");

    let info = fetch_media_info(&player)
        .await
        .expect("playback status remains available");
    assert_eq!(info.art_source, None);
}

#[tokio::test]
async fn browser_bridge_ingestion_keeps_owner_and_source_pids_separate() {
    let fixture = MprisFixture::start_with_kde_pid(SOURCE_BROWSER_PID).await;
    let player = build_player_state(
        &fixture.client,
        TEST_BRIDGE_PLAYER_NAME,
        &MediaConfig::default(),
    )
    .await
    .expect("build bridge player")
    .expect("bridge owner should remain stable");

    let info = fetch_media_info(&player)
        .await
        .expect("bridge metadata should be readable");

    assert_eq!(info.owner_pid, Some(std::process::id()));
    assert_eq!(info.source_pid_hint, Some(SOURCE_BROWSER_PID));
    assert_eq!(
        info.browser_family, None,
        "the fixture keeps the bridge family unresolved so source-PID fallback is exercised"
    );

    let ordinary = MprisFixture::start().await;
    let ordinary_player =
        build_player_state(&ordinary.client, TEST_PLAYER_NAME, &MediaConfig::default())
            .await
            .expect("build ordinary player")
            .expect("ordinary owner should remain stable");
    let ordinary_info = fetch_media_info(&ordinary_player)
        .await
        .expect("ordinary metadata should be readable");
    assert_eq!(ordinary_info.source_pid_hint, None);
}
