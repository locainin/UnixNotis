use std::collections::HashMap;

use super::super::command::handle_command;
use super::super::player::PlayerState;
use super::support::{build_player_state, MprisFixture, TEST_PLAYER_NAME};
use crate::media::MediaCommand;
use unixnotis_core::MediaConfig;

#[tokio::test]
async fn command_dispatch_is_a_noop_for_refresh_and_missing_players() {
    let players = HashMap::<String, PlayerState>::new();

    assert!(handle_command(&players, MediaCommand::Refresh)
        .await
        .expect("refresh command")
        .is_none());
    assert!(handle_command(
        &players,
        MediaCommand::PlayPause {
            bus_name: "org.mpris.MediaPlayer2.missing".to_string(),
        },
    )
    .await
    .expect("missing player command")
    .is_none());
}

#[tokio::test]
async fn command_dispatch_calls_the_selected_live_player() {
    let fixture = MprisFixture::start().await;
    let player = build_player_state(&fixture.client, TEST_PLAYER_NAME, &MediaConfig::default())
        .await
        .expect("probe test MPRIS player")
        .expect("stable test MPRIS owner");
    let players = HashMap::from([(TEST_PLAYER_NAME.to_string(), player)]);

    let refreshed_player = handle_command(
        &players,
        MediaCommand::Next {
            bus_name: TEST_PLAYER_NAME.to_string(),
        },
    )
    .await
    .expect("send next command");

    assert_eq!(refreshed_player.as_deref(), Some(TEST_PLAYER_NAME));
    assert_eq!(fixture.next_calls(), 1);
}
