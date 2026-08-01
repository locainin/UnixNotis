use super::super::constants::MAX_MPRIS_PROPERTY_REPLY_BYTES;
use super::super::metadata::{metadata_entry_count_allowed, property_reply_body_allowed};
use super::super::{build_player_state, fetch_media_info};
use super::support::{MprisFixture, TEST_PLAYER_NAME};
use unixnotis_core::MediaConfig;

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
