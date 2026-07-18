use std::collections::HashMap;

use super::super::command::handle_command;
use super::super::player::PlayerState;
use crate::media::MediaCommand;

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
