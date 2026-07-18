use std::collections::HashMap;

use super::super::refresh::prune_player_refreshes;

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
